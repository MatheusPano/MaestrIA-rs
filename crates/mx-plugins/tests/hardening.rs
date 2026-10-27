mod common;

use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use common::{SH_PLUGIN, plugins_in, until, write_plugin};
use mx_plugins::process::{ConnHandlers, Connection};
use mx_plugins::{PluginListener, PluginState, Plugins};
use serde_json::{Value, json};

fn alive(pid: &str) -> bool {
    fs::read_to_string(format!("/proc/{pid}/stat"))
        .is_ok_and(|stat| stat.rsplit(')').next().is_some_and(|rest| !rest.trim_start().starts_with('Z')))
}

async fn read_pid(file: &Path) -> String {
    assert!(until(|| fs::read_to_string(file).is_ok_and(|t| t.ends_with('\n'))).await);
    fs::read_to_string(file).unwrap().trim().to_owned()
}

fn counting_handlers(requests: Arc<AtomicUsize>) -> ConnHandlers {
    ConnHandlers {
        on_request: Arc::new(move |_, _| {
            requests.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Ok(Value::Null) })
        }),
        on_log: Arc::new(|_| {}),
        on_exit: Box::new(|_| {}),
    }
}

fn spawn(dir: &Path, script: &str, requests: Arc<AtomicUsize>) -> Arc<Connection> {
    let dir = write_plugin(dir, "c", json!({}), &[("p.sh", script)]);
    let env: Vec<(String, String)> = std::env::vars().collect();
    Connection::spawn(&dir.join("p.sh"), &[], &dir, &env, counting_handlers(requests)).unwrap()
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn stopping_a_wrapper_plugin_takes_down_the_children_it_started() {
    let tmp = tempfile::tempdir().unwrap();
    let pidfile = tmp.path().join("child");
    let script = format!(
        "#!/bin/sh\ntrap '' TERM\nsleep 300 &\necho $! > '{}'\nwhile :; do sleep 1; done\n",
        pidfile.display()
    );
    let conn = spawn(tmp.path(), &script, Arc::default());
    let child = read_pid(&pidfile).await;
    assert!(alive(&child));
    conn.close(Duration::from_millis(200)).await;
    assert!(!conn.alive());
    assert!(until(|| !alive(&child)).await);
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn kill_reaches_the_whole_process_group() {
    let tmp = tempfile::tempdir().unwrap();
    let pidfile = tmp.path().join("child");
    let script = format!("#!/bin/sh\nsleep 300 &\necho $! > '{}'\nwait\n", pidfile.display());
    let conn = spawn(tmp.path(), &script, Arc::default());
    let child = read_pid(&pidfile).await;
    conn.kill();
    assert!(until(|| !conn.alive()).await);
    assert!(until(|| !alive(&child)).await);
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn an_escaped_child_holding_stdout_is_not_heard_after_stop() {
    let tmp = tempfile::tempdir().unwrap();
    let pidfile = tmp.path().join("child");
    let script = format!(
        "#!/bin/sh\nsetsid sh -c 'echo $$ > \"{}\"; while :; do echo \"{{\\\"jsonrpc\\\":\\\"2.0\\\",\\\"method\\\":\\\"log\\\"}}\"; sleep 0.05; done' &\nwhile read -r l; do case \"$l\" in *shutdown*) exit 0;; esac; done\n",
        pidfile.display()
    );
    let requests = Arc::new(AtomicUsize::new(0));
    let conn = spawn(tmp.path(), &script, requests.clone());
    let child = read_pid(&pidfile).await;
    assert!(until(|| requests.load(Ordering::SeqCst) > 0).await);
    conn.close(Duration::from_millis(300)).await;
    assert!(!conn.alive());
    tokio::time::sleep(Duration::from_millis(100)).await;
    let heard = requests.load(Ordering::SeqCst);
    tokio::time::sleep(Duration::from_millis(400)).await;
    let after = requests.load(Ordering::SeqCst);
    let _ = nix::sys::signal::kill(
        nix::unistd::Pid::from_raw(child.parse().unwrap()),
        nix::sys::signal::Signal::SIGKILL,
    );
    assert_eq!(after, heard);
}

#[tokio::test]
async fn a_plugin_that_stops_reading_does_not_grow_the_queue_without_bound() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = write_plugin(tmp.path(), "c", json!({}), &[("p.sh", "#!/bin/sh\nsleep 300\n")]);
    let env: Vec<(String, String)> = std::env::vars().collect();
    let dropped = Arc::new(AtomicUsize::new(0));
    let counter = dropped.clone();
    let handlers = ConnHandlers {
        on_request: Arc::new(|_, _| Box::pin(async { Ok(Value::Null) })),
        on_log: Arc::new(move |line| {
            if line.contains("não está lendo") {
                counter.fetch_add(1, Ordering::SeqCst);
            }
        }),
        on_exit: Box::new(|_| {}),
    };
    let conn = Connection::spawn(&dir.join("p.sh"), &[], &dir, &env, handlers).unwrap();
    let big = "x".repeat(64 * 1024);
    let sent = mx_plugins::process::OUTBOX_LINES * 4;
    for _ in 0..sent {
        conn.notify("event", Some(json!({ "pad": big })));
    }
    let dropped = dropped.load(Ordering::SeqCst);
    assert!(dropped >= sent - mx_plugins::process::OUTBOX_LINES - 2);
    conn.kill();
    assert!(until(|| !conn.alive()).await);
}

struct DropMainOnStart {
    plugins: OnceLock<Plugins>,
    manifest: std::path::PathBuf,
    done: AtomicBool,
}

impl PluginListener for DropMainOnStart {
    fn changed(&self) {
        if self.done.swap(true, Ordering::SeqCst) {
            return;
        }
        fs::write(&self.manifest, json!({"id": "sh", "version": "2"}).to_string()).unwrap();
        self.plugins.get().unwrap().scan();
    }
}

#[tokio::test]
async fn a_rescan_that_drops_main_while_starting_leaves_the_plugin_idle() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = write_plugin(
        &tmp.path().join("plugins"),
        "sh",
        json!({"id": "sh", "version": "1", "main": ["./plugin.sh"]}),
        &[("plugin.sh", SH_PLUGIN)],
    );
    let plugins = plugins_in(tmp.path());
    plugins.scan();
    let listener = Arc::new(DropMainOnStart {
        plugins: OnceLock::new(),
        manifest: dir.join(mx_plugins::MANIFEST_NAME),
        done: AtomicBool::new(false),
    });
    let _ = listener.plugins.set(plugins.clone());
    plugins.set_listener(listener.clone());
    assert!(plugins.start("sh").await.is_none());
    assert!(listener.done.load(Ordering::SeqCst));
    assert_eq!(plugins.get("sh").unwrap().state, PluginState::Idle);
}

#[tokio::test]
async fn an_update_that_cannot_be_moved_into_place_keeps_the_installed_version() {
    let tmp = tempfile::tempdir().unwrap();
    let src = write_plugin(tmp.path(), "fonte", json!({"id": "meu.plugin", "version": "1.0.0"}), &[]);
    let plugins = plugins_in(tmp.path());
    let root = tmp.path().join("plugins");
    let staged = plugins.stage(src.to_str().unwrap()).await.unwrap();
    plugins.commit(&staged).await.unwrap();

    fs::write(src.join("maestria-plugin.json"), json!({"id": "meu.plugin", "version": "1.1.0"}).to_string()).unwrap();
    let staged = plugins.stage(src.to_str().unwrap()).await.unwrap();
    fs::remove_dir_all(&staged.dir).unwrap();
    assert!(plugins.commit(&staged).await.is_err());
    assert!(root.join("meu.plugin/maestria-plugin.json").exists());
    let all = plugins.all();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].manifest.as_ref().unwrap().version, "1.0.0");
    assert!(!staged.staging.exists());
}
