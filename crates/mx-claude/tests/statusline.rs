mod common;

use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Output};
use std::time::{Duration, Instant};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use common::start;
use mx_claude::{STATUS_LINE_SCRIPT, sh_quote, status_line_file, user_status_line};
use serde_json::{Value, json};

fn settings_at(base: &Path, name: &str, body: &str) {
    let dir = base.join(".claude");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(name), body).unwrap();
}

fn line(command: &str) -> String {
    json!({"statusLine": {"type": "command", "command": command}}).to_string()
}

fn status_line_in(settings: &str) -> Value {
    serde_json::from_str::<Value>(settings).unwrap()["statusLine"].clone()
}

fn args_of(command: &str) -> Vec<String> {
    let out = Command::new("sh")
        .arg("-c")
        .arg(format!(
            r#"f() {{ for a in "$@"; do printf "%s\0" "$a"; done; }}; f {command}"#
        ))
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8(out.stdout).unwrap();
    text[..text.len() - 1]
        .split('\0')
        .map(str::to_owned)
        .collect()
}

fn dirs() -> (tempfile::TempDir, String, String) {
    let tmp = tempfile::tempdir().unwrap();
    let cwd = tmp.path().join("proj");
    let home = tmp.path().join("home");
    std::fs::create_dir_all(&cwd).unwrap();
    std::fs::create_dir_all(&home).unwrap();
    let s = |p: &Path| p.to_string_lossy().into_owned();
    (tmp, s(&cwd), s(&home))
}

fn user_command(cwd: &str, home: &str) -> Option<String> {
    user_status_line(Some(cwd), Some(home))
        .and_then(|l| l.get("command").and_then(Value::as_str).map(str::to_owned))
}

#[test]
fn project_local_beats_project_which_beats_home() {
    let (_tmp, cwd, home) = dirs();
    settings_at(Path::new(&home), "settings.json", &line("home"));
    assert_eq!(user_command(&cwd, &home).as_deref(), Some("home"));
    settings_at(Path::new(&cwd), "settings.json", &line("projeto"));
    assert_eq!(user_command(&cwd, &home).as_deref(), Some("projeto"));
    settings_at(Path::new(&cwd), "settings.local.json", &line("local"));
    assert_eq!(user_command(&cwd, &home).as_deref(), Some("local"));
}

#[test]
fn invalid_json_is_skipped() {
    let (_tmp, cwd, home) = dirs();
    settings_at(Path::new(&cwd), "settings.local.json", "{ isto não é json");
    settings_at(Path::new(&home), "settings.json", &line("home"));
    assert_eq!(user_command(&cwd, &home).as_deref(), Some("home"));
}

#[test]
fn status_line_of_another_type_is_ignored() {
    let (_tmp, cwd, home) = dirs();
    let other = json!({"statusLine": {"type": "outro", "command": "x"}}).to_string();
    settings_at(Path::new(&cwd), "settings.json", &other);
    assert_eq!(user_status_line(Some(&cwd), Some(&home)), None);
}

#[test]
fn nothing_anywhere_is_none() {
    let (_tmp, cwd, home) = dirs();
    assert_eq!(user_status_line(Some(&cwd), Some(&home)), None);
    assert_eq!(user_status_line(None, Some(&home)), None);
}

#[tokio::test]
async fn without_user_status_line_the_script_gets_an_empty_third_argument() {
    let (_tmp, cwd, home) = dirs();
    let state = tempfile::tempdir().unwrap();
    let (server, _rx) = start(state.path()).await;
    let settings = server.settings_for("t1", Some(&cwd), Some(&home));
    let status = status_line_in(&settings);
    assert_eq!(status["type"], "command");
    assert!(status.get("padding").is_none());
    let command = status["command"].as_str().unwrap();
    let port = server.port();
    assert_eq!(
        args_of(command),
        [
            "sh".to_owned(),
            status_line_file(state.path())
                .to_string_lossy()
                .into_owned(),
            format!("http://127.0.0.1:{port}/status/t1"),
            String::new(),
        ]
    );
    assert!(command.ends_with(" ''"));
    let json: Value = serde_json::from_str(&settings).unwrap();
    let hooks = json["hooks"].as_object().unwrap();
    for key in ["SessionStart", "Stop", "PreToolUse", "SubagentStop"] {
        assert!(hooks.contains_key(key), "{key}");
    }
    assert_eq!(
        hooks["Stop"][0]["hooks"][0]["url"],
        format!("http://127.0.0.1:{port}/hook/t1")
    );
}

#[tokio::test]
async fn user_command_travels_in_base64_with_its_padding() {
    let (_tmp, cwd, home) = dirs();
    let user = r#"echo "$HOME" | tr a-z A-Z"#;
    let body = json!({"statusLine": {"type": "command", "command": user, "padding": 2}});
    settings_at(Path::new(&cwd), "settings.json", &body.to_string());
    let state = tempfile::tempdir().unwrap();
    let (server, _rx) = start(state.path()).await;
    let status = status_line_in(&server.settings_for("t1", Some(&cwd), Some(&home)));
    assert_eq!(status["padding"], 2);
    let args = args_of(status["command"].as_str().unwrap());
    assert_eq!(args[3], STANDARD.encode(user));
}

#[tokio::test]
async fn script_is_written_when_the_server_starts() {
    let state = tempfile::tempdir().unwrap();
    let (_server, _rx) = start(state.path()).await;
    let written = std::fs::read_to_string(status_line_file(state.path())).unwrap();
    assert_eq!(written, STATUS_LINE_SCRIPT);
}

#[tokio::test]
async fn survives_both_quoting_layers_down_to_the_claude_shell() {
    let (_tmp, cwd, home) = dirs();
    let user = r#"printf '%s' "it's $x" | tr a-z A-Z"#;
    settings_at(Path::new(&cwd), "settings.json", &line(user));
    let state = tempfile::tempdir().unwrap();
    let (server, _rx) = start(state.path()).await;
    let settings = server.settings_for("t1", Some(&cwd), Some(&home));
    let out = Command::new("sh")
        .arg("-c")
        .arg(format!(r#"printf "%s" {}"#, sh_quote(&settings)))
        .output()
        .unwrap();
    let echoed = String::from_utf8(out.stdout).unwrap();
    assert_eq!(echoed, settings);
    let args = args_of(status_line_in(&echoed)["command"].as_str().unwrap());
    assert_eq!(STANDARD.decode(&args[3]).unwrap(), user.as_bytes());
}

const JSON: &str = r#"{"model":{"display_name":"Opus"},"context_window":{"used_percentage":42}}"#;
const CLOSED: &str = "http://127.0.0.1:1/status/t1";

fn b64(s: &str) -> String {
    STANDARD.encode(s)
}

fn run_script(dir: &Path, url: &str, user: &str, path_env: Option<&str>) -> Output {
    let script = dir.join("statusline.sh");
    std::fs::write(&script, STATUS_LINE_SCRIPT).unwrap();
    let mut cmd = Command::new("/bin/sh");
    cmd.arg("-c").arg(format!(
        "printf '%s' {} | /bin/sh {} {} {}",
        sh_quote(JSON),
        sh_quote(&script.to_string_lossy()),
        sh_quote(url),
        sh_quote(user)
    ));
    if let Some(path) = path_env {
        cmd.env_clear().env("PATH", path);
    }
    cmd.output().unwrap()
}

#[test]
fn with_user_cat_the_output_is_the_received_json() {
    let dir = tempfile::tempdir().unwrap();
    let out = run_script(dir.path(), CLOSED, &b64("cat"), None);
    assert_eq!(String::from_utf8(out.stdout).unwrap(), JSON);
    assert!(out.status.success());
}

#[test]
fn without_user_command_output_is_empty_and_exit_zero() {
    let dir = tempfile::tempdir().unwrap();
    let out = run_script(dir.path(), CLOSED, "", None);
    assert!(out.stdout.is_empty());
    assert!(out.status.success());
}

#[test]
fn quotes_dollar_and_pipe_behave_like_running_directly() {
    let dir = tempfile::tempdir().unwrap();
    let user = r#"printf "%s" 'a$b' | tr a b"#;
    let direct = Command::new("/bin/sh")
        .arg("-c")
        .arg(user)
        .output()
        .unwrap();
    assert_eq!(direct.stdout, b"b$b");
    let out = run_script(dir.path(), CLOSED, &b64(user), None);
    assert_eq!(out.stdout, direct.stdout);
}

#[test]
fn json_reaches_the_server_at_the_url() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let got = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut raw = Vec::new();
        let mut buf = [0u8; 4096];
        while let Ok(n) = stream.read(&mut buf) {
            if n == 0 {
                break;
            }
            raw.extend_from_slice(&buf[..n]);
            if String::from_utf8_lossy(&raw).ends_with(JSON) {
                break;
            }
        }
        let _ = stream.write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 0\r\n\r\n");
        String::from_utf8(raw).unwrap()
    });
    let dir = tempfile::tempdir().unwrap();
    run_script(
        dir.path(),
        &format!("http://127.0.0.1:{port}/status/t1"),
        &b64("cat"),
        None,
    );
    let request = got.join().unwrap();
    let body = request.split("\r\n\r\n").nth(1).unwrap();
    assert_eq!(body, JSON);
}

#[test]
fn closed_port_finishes_fast_with_the_user_output_intact() {
    let dir = tempfile::tempdir().unwrap();
    let started = Instant::now();
    let out = run_script(dir.path(), CLOSED, &b64("cat"), None);
    assert!(started.elapsed() < Duration::from_millis(1500));
    assert_eq!(String::from_utf8(out.stdout).unwrap(), JSON);
}

#[test]
fn without_curl_on_path_nothing_reaches_stderr() {
    let dir = tempfile::tempdir().unwrap();
    let bin = dir.path().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    for tool in ["sh", "cat", "base64", "tr", "printf"] {
        let found = Command::new("/bin/sh")
            .arg("-c")
            .arg(format!("command -v {tool}"))
            .output()
            .unwrap();
        let path = String::from_utf8(found.stdout).unwrap().trim().to_owned();
        if path.starts_with('/') {
            std::os::unix::fs::symlink(&path, bin.join(tool)).unwrap();
        }
    }
    assert!(!bin.join("curl").exists());
    let out = run_script(
        dir.path(),
        CLOSED,
        &b64("cat"),
        Some(&bin.to_string_lossy()),
    );
    assert_eq!(String::from_utf8(out.stdout).unwrap(), JSON);
    assert!(
        out.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.status.success());
}
