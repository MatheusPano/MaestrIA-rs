mod common;

use std::time::Duration;

use common::{post, start, stop_url_port};
use mx_claude::{ServerEvent, port_file};

#[tokio::test]
async fn second_run_binds_the_same_port_as_the_first() {
    let state = tempfile::tempdir().unwrap();
    let (mut first, _rx) = start(state.path()).await;
    let was = first.port();
    assert!(was > 0);
    first.stop().await;

    let (again, _rx2) = start(state.path()).await;
    assert_eq!(again.port(), was);
    assert_eq!(stop_url_port(&again.settings_for("tab2", None, None)), was);
}

#[tokio::test]
async fn occupied_saved_port_falls_back_to_a_free_one_and_remembers_it() {
    let state = tempfile::tempdir().unwrap();
    let squatter = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let taken = squatter.local_addr().unwrap().port();
    std::fs::write(port_file(state.path()), format!("{taken}\n")).unwrap();

    let (server, _rx) = start(state.path()).await;
    assert_ne!(server.port(), taken);
    let saved = std::fs::read_to_string(port_file(state.path())).unwrap();
    assert_eq!(saved.trim(), server.port().to_string());
}

#[tokio::test]
async fn unreadable_port_file_does_not_stop_the_server() {
    let state = tempfile::tempdir().unwrap();
    std::fs::write(port_file(state.path()), "nem número\n").unwrap();

    let (server, _rx) = start(state.path()).await;
    assert!(server.port() > 0);
    let saved = std::fs::read_to_string(port_file(state.path())).unwrap();
    assert_eq!(saved, format!("{}\n", server.port()));
}

#[tokio::test]
async fn hook_reaches_the_panel_through_the_announced_port() {
    let state = tempfile::tempdir().unwrap();
    let (server, mut rx) = start(state.path()).await;
    let port = stop_url_port(&server.settings_for("tab7", None, None));
    assert_eq!(port, server.port());

    let (status, body) = post(
        port,
        "/hook/tab7",
        r#"{"hook_event_name":"Stop","session_id":"s1"}"#,
    )
    .await;
    assert_eq!((status, body.as_str()), (200, "{}"));

    let event = tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .unwrap()
        .unwrap();
    match event {
        ServerEvent::Hook {
            tab_id,
            name,
            payload,
        } => {
            assert_eq!(tab_id, "tab7");
            assert_eq!(name, "Stop");
            assert_eq!(payload["session_id"], "s1");
        }
        other => panic!("unexpected {other:?}"),
    }
}

#[tokio::test]
async fn malformed_or_nameless_hooks_still_answer_an_empty_decision() {
    let state = tempfile::tempdir().unwrap();
    let (server, mut rx) = start(state.path()).await;
    for (path, body) in [
        ("/hook/tab1", "not json"),
        ("/hook/tab1", "[1,2]"),
        ("/hook/tab1", ""),
        ("/hook", r#"{"hook_event_name":"Stop"}"#),
    ] {
        let (status, text) = post(server.port(), path, body).await;
        assert_eq!((status, text.as_str()), (200, "{}"));
    }
    let res = reqwest::get(format!("http://127.0.0.1:{}/hook/tab1", server.port()))
        .await
        .unwrap();
    assert_eq!(
        res.headers()["content-type"],
        "application/json; charset=utf-8"
    );
    assert!(rx.try_recv().is_err());
}

#[tokio::test]
async fn any_first_segment_other_than_mcp_or_status_is_a_hook() {
    let state = tempfile::tempdir().unwrap();
    let (server, mut rx) = start(state.path()).await;
    post(
        server.port(),
        "/whatever/tab1",
        r#"{"hook_event_name":"Stop"}"#,
    )
    .await;
    assert!(matches!(rx.recv().await, Some(ServerEvent::Hook { tab_id, .. }) if tab_id == "tab1"));
}
