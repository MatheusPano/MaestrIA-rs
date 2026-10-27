use std::sync::Arc;

use mx_sys::notify::{Notifier, alert_command, applescript_string};
use mx_sys::reveal::{encode_full, open_link_on, quick_look_on, reveal_on, show_items_command};
use mx_sys::runner::Call;
use mx_sys::{FakeRunner, Platform, ShResult, editor, output};

fn ok() -> ShResult {
    output(0, "", "")
}

#[test]
fn linux_alert_is_notify_send_with_quoted_title_and_body() {
    assert_eq!(
        alert_command(Platform::Linux, "MaestrIA", "api · it's waiting"),
        r"notify-send -a maestria -- 'MaestrIA' 'api · it'\''s waiting'"
    );
}

#[test]
fn mac_alert_is_osascript_with_applescript_escaping() {
    assert_eq!(applescript_string(r#"a "b" \c"#), r#""a \"b\" \\c""#);
    assert_eq!(
        alert_command(Platform::MacOs, "MaestrIA", "x"),
        r#"osascript -e 'display notification "x" with title "MaestrIA"'"#
    );
}

#[tokio::test]
async fn alert_runs_through_the_runner_without_cwd() {
    let fake = Arc::new(FakeRunner::new());
    Notifier::on(fake.clone(), Platform::Linux).alert("MaestrIA", "t · c").await;
    assert_eq!(
        fake.calls(),
        vec![Call { command: "notify-send -a maestria -- 'MaestrIA' 't · c'".into(), cwd: None }]
    );
}

#[test]
fn badge_is_deduplicated_and_only_mac_gets_a_label_to_stamp() {
    let mac = Notifier::on(Arc::new(FakeRunner::new()), Platform::MacOs);
    assert_eq!(mac.badge(None), None);
    assert_eq!(mac.badge(Some("2")).as_deref(), Some("2"));
    assert_eq!(mac.badge(Some("2")), None);
    assert_eq!(mac.badge(None).as_deref(), Some(""));
    let linux = Notifier::on(Arc::new(FakeRunner::new()), Platform::Linux);
    assert_eq!(linux.badge(Some("1")), None);
}

#[test]
fn encode_full_keeps_reserved_chars_and_escapes_the_rest_as_utf8() {
    assert_eq!(encode_full("/a b/ção#?.md"), "/a%20b/%C3%A7%C3%A3o#?.md");
    assert_eq!(encode_full("100%"), "100%25");
}

#[test]
fn show_items_is_the_exact_dbus_call() {
    assert_eq!(
        show_items_command("/tmp/a b.md"),
        "dbus-send --session --print-reply --dest=org.freedesktop.FileManager1 \
/org/freedesktop/FileManager1 org.freedesktop.FileManager1.ShowItems \
array:string:'file:///tmp/a%20b.md' string:\"\""
    );
}

#[tokio::test]
async fn linux_reveal_falls_back_to_opening_the_parent() {
    let fake = FakeRunner::new();
    fake.respond("xdg-open", ok());
    assert!(reveal_on(&fake, Platform::Linux, "/tmp/x/a.md").await);
    let cmds = fake.commands();
    assert!(cmds[0].starts_with("dbus-send "));
    assert_eq!(cmds[1], "xdg-open '/tmp/x'");
}

#[tokio::test]
async fn mac_reveal_is_open_dash_r() {
    let fake = FakeRunner::new();
    fake.respond("open -R", ok());
    assert!(reveal_on(&fake, Platform::MacOs, "/a.md").await);
    assert_eq!(fake.commands(), ["open -R '/a.md'"]);
}

#[tokio::test]
async fn linux_quick_look_needs_an_existing_file_then_xdg_opens_it() {
    let fake = FakeRunner::new().with_fallback(ok());
    assert!(!quick_look_on(&fake, Platform::Linux, "/nao/existe.md").await);
    assert!(fake.calls().is_empty());
    let file = tempfile::NamedTempFile::new().unwrap();
    let path = file.path().to_str().unwrap();
    assert!(quick_look_on(&fake, Platform::Linux, path).await);
    assert_eq!(fake.commands(), [format!("xdg-open '{path}'")]);
}

#[tokio::test]
async fn open_link_uses_xdg_open_or_open() {
    let fake = FakeRunner::new().with_fallback(ok());
    assert!(open_link_on(&fake, Platform::Linux, "https://x.y/?a=1").await);
    assert!(open_link_on(&fake, Platform::MacOs, "https://x.y").await);
    assert_eq!(fake.commands(), ["xdg-open 'https://x.y/?a=1'", "open 'https://x.y'"]);
}

#[tokio::test]
async fn editor_opens_with_code_and_jumps_to_a_line_with_dash_g() {
    let fake = FakeRunner::new().with_fallback(ok());
    assert!(editor::open_on(&fake, Platform::Linux, "/r", None).await);
    assert!(editor::open_on(&fake, Platform::Linux, "/r/a.rs", Some(12)).await);
    assert_eq!(fake.commands(), ["code -- '/r'", "code -g -- '/r/a.rs:12'"]);
}

#[tokio::test]
async fn editor_on_linux_falls_back_to_insiders_then_xdg_open() {
    let fake = FakeRunner::new();
    fake.respond("xdg-open", ok());
    assert!(editor::open_on(&fake, Platform::Linux, "/r", None).await);
    assert_eq!(fake.commands(), ["code -- '/r'", "code-insiders -- '/r'", "xdg-open '/r'"]);
    let none = FakeRunner::new();
    assert!(!editor::open_on(&none, Platform::Linux, "/r", None).await);
}

#[tokio::test]
async fn editor_on_mac_falls_back_to_open_dash_a() {
    let fake = FakeRunner::new();
    fake.respond("open -a", ok());
    assert!(editor::open_on(&fake, Platform::MacOs, "/r", None).await);
    assert_eq!(fake.commands(), ["code -- '/r'", "open -a 'Visual Studio Code' '/r'"]);
}

#[tokio::test]
async fn operands_that_look_like_options_are_never_read_as_options() {
    assert_eq!(
        alert_command(Platform::Linux, "-t 1", "--help"),
        "notify-send -a maestria -- '-t 1' '--help'"
    );
    let fake = FakeRunner::new();
    fake.respond("xdg-open", ok());
    fake.respond("open", ok());
    assert!(editor::open_on(&fake, Platform::Linux, "-r", Some(3)).await);
    assert!(open_link_on(&fake, Platform::Linux, "--help").await);
    assert!(reveal_on(&fake, Platform::MacOs, "-x").await);
    assert_eq!(
        fake.commands(),
        [
            "code -g -- '-r:3'",
            "code-insiders -- '-r'",
            "xdg-open './-r'",
            "xdg-open './--help'",
            "open -R './-x'"
        ]
    );
}
