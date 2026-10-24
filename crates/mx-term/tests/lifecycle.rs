use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use mx_core::ports::TermFactory;
use mx_term::{GridSize, SpawnOptions, TermEvent, TermRegistry, TerminalSession};
use mx_theme::TerminalPalette;

const SIZE: GridSize = GridSize { cols: 60, rows: 12, cell_width: 8, cell_height: 16 };

fn until(limit: Duration, mut f: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + limit;
    while Instant::now() < deadline {
        if f() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    f()
}

fn alive(pid: u32) -> bool {
    std::fs::read_to_string(format!("/proc/{pid}/stat"))
        .is_ok_and(|s| s.rsplit(')').next().is_some_and(|rest| !rest.trim_start().starts_with('Z')))
}

fn orphan_script(dir: &Path) -> (String, PathBuf) {
    let pidfile = dir.join("orphan.pid");
    let script = dir.join("orphan.sh");
    let body = format!(
        "(trap '' HUP; while printf tick; do sleep 0.05; done) &\necho $! > '{}'\nexit 0\n",
        pidfile.display()
    );
    std::fs::write(&script, body).unwrap();
    (format!("sh '{}'", script.display()), pidfile)
}

fn read_pid(file: &Path) -> u32 {
    assert!(until(Duration::from_secs(5), || std::fs::read_to_string(file).is_ok_and(|s| s.ends_with('\n'))));
    std::fs::read_to_string(file).unwrap().trim().parse().unwrap()
}

fn screen(s: &TerminalSession) -> String {
    (0..SIZE.rows as usize).map(|r| s.row_text(r)).collect::<Vec<_>>().join("\n")
}

#[test]
fn closing_a_tab_whose_leader_exited_closes_the_pty_for_background_jobs() {
    let dir = tempfile::tempdir().unwrap();
    let (script, pidfile) = orphan_script(dir.path());
    let registry = TermRegistry::new(TerminalPalette::default());
    let mut term = registry.create("orphan");
    term.start_command(&script, "/", None);
    let orphan = read_pid(&pidfile);
    assert!(until(Duration::from_secs(5), || term.exited()));
    assert!(alive(orphan));
    drop(term);
    assert!(registry.get("orphan").is_none());
    assert!(until(Duration::from_secs(5), || !alive(orphan)), "orphan {orphan} survived the closed tab");
}

#[test]
fn relaunch_stops_the_previous_life_from_feeding_the_terminal() {
    let dir = tempfile::tempdir().unwrap();
    let (script, pidfile) = orphan_script(dir.path());
    let registry = TermRegistry::new(TerminalPalette::default());
    let mut term = registry.create("relaunch");
    term.start_command(&script, "/", None);
    let orphan = read_pid(&pidfile);
    let s = registry.get("relaunch").unwrap();
    assert!(until(Duration::from_secs(5), || term.exited()));
    term.relaunch("printf fresh; sleep 5", "/");
    assert!(until(Duration::from_secs(5), || screen(&s).contains("fresh")));
    assert!(until(Duration::from_secs(5), || !alive(orphan)), "orphan {orphan} kept running after relaunch");
    s.feed(b"\x1b[2J\x1b[H");
    std::thread::sleep(Duration::from_millis(300));
    assert!(!screen(&s).contains("tick"), "{}", screen(&s));
    futures_kill(&s);
}

#[test]
fn a_hung_up_child_that_stays_alive_does_not_spin_the_io_thread() {
    let options = SpawnOptions {
        program: Some("/bin/sh".into()),
        args: vec!["-c".into(), "exec 0<&- 1>&- 2>&-; sleep 2".into()],
        ..Default::default()
    };
    let t = TerminalSession::spawn(options, SIZE, TerminalPalette::default()).unwrap();
    std::thread::sleep(Duration::from_millis(300));
    let before = thread_cpu("mx-pty");
    std::thread::sleep(Duration::from_millis(1000));
    let used = thread_cpu("mx-pty").saturating_sub(before);
    assert!(used < 20, "mx-pty used {used} ticks in 1 s");
    assert!(until(Duration::from_secs(5), || t.is_exited()));
}

#[test]
fn a_view_subscription_sees_wakeups_even_after_an_older_one_went_away() {
    let t = TerminalSession::new(SIZE, TerminalPalette::default(), 100);
    let old = t.subscribe();
    t.feed(b"one");
    drop(old);
    let view = t.subscribe();
    assert!(view.dirty.take());
    t.feed(b"two");
    assert_eq!(view.events.try_recv().ok(), Some(TermEvent::Wakeup));
    assert!(view.dirty.take());
}

#[test]
fn combining_marks_reach_the_snapshot() {
    let t = TerminalSession::new(SIZE, TerminalPalette::default(), 100);
    t.feed("cafe\u{301}".as_bytes());
    let snap = t.snapshot();
    let cell = snap.row(0)[3];
    assert_eq!(cell.c, 'e');
    assert_eq!(cell.marks.chars().collect::<String>(), "\u{301}");
}

fn thread_cpu(prefix: &str) -> u64 {
    std::fs::read_dir("/proc/self/task")
        .map(|d| {
            d.filter_map(Result::ok)
                .filter(|t| std::fs::read_to_string(t.path().join("comm")).is_ok_and(|c| c.starts_with(prefix)))
                .filter_map(|t| std::fs::read_to_string(t.path().join("stat")).ok())
                .filter_map(|stat| {
                    let rest = stat.rsplit(')').next()?.split_whitespace().collect::<Vec<_>>();
                    Some(rest.get(11)?.parse::<u64>().ok()? + rest.get(12)?.parse::<u64>().ok()?)
                })
                .sum()
        })
        .unwrap_or(0)
}

fn futures_kill(s: &TerminalSession) {
    let done = s.kill_and_wait();
    let _ = done.recv_blocking();
}
