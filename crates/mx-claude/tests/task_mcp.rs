mod common;

use std::sync::Arc;

use common::{Desk, post, start};
use mx_claude::mcp::{self, NoSink, SuggestionSink, is_task_id, new_task_id};
use serde_json::{Value, json};

fn call(tool: &str, args: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": tool, "arguments": args}})
}

fn text_of(response: &Value) -> &str {
    response["result"]["content"][0]["text"].as_str().unwrap()
}

async fn handle(body: Value, tab: &str, sink: &dyn SuggestionSink) -> Option<Value> {
    mcp::handle(&body, tab, sink).await
}

#[tokio::test]
async fn initialize_echoes_the_client_version_and_introduces_itself() {
    let body = json!({"jsonrpc": "2.0", "id": 0, "method": "initialize", "params": {"protocolVersion": "2025-03-26", "capabilities": {}}});
    let r = handle(body, "tab1", &Desk::default()).await.unwrap();
    let result = &r["result"];
    assert_eq!(result["protocolVersion"], "2025-03-26");
    assert_eq!(result["serverInfo"]["name"], "maestria");
    assert!(result["capabilities"].get("tools").is_some());
    assert!(
        result["instructions"]
            .as_str()
            .unwrap()
            .contains("suggest_task")
    );
}

#[tokio::test]
async fn initialize_without_a_version_answers_ours() {
    let body = json!({"jsonrpc": "2.0", "id": 0, "method": "initialize"});
    let r = handle(body, "tab1", &NoSink).await.unwrap();
    assert_eq!(r["result"]["protocolVersion"], "2025-06-18");
}

#[tokio::test]
async fn notification_has_no_answer() {
    let body = json!({"jsonrpc": "2.0", "method": "notifications/initialized"});
    assert_eq!(handle(body, "tab1", &Desk::default()).await, None);
}

#[tokio::test]
async fn lists_both_tools_with_what_each_requires() {
    let r = handle(
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
        "tab1",
        &NoSink,
    )
    .await
    .unwrap();
    let tools = r["result"]["tools"].as_array().unwrap();
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["suggest_task", "dismiss_task"]);
    assert_eq!(
        tools[0]["inputSchema"]["required"],
        json!(["title", "tldr", "prompt"])
    );
}

#[tokio::test]
async fn a_suggestion_is_queued_and_returns_the_id_to_withdraw_later() {
    let desk = Desk::default();
    let args = json!({"title": "Converter os exemplos SQL", "tldr": "O guia está em PostgreSQL.", "prompt": "  Reescreva as seções 3 e 5 em MySQL.  "});
    let r = handle(call("suggest_task", args), "tab1", &desk)
        .await
        .unwrap();
    assert_eq!(desk.pending()[0].1, "Reescreva as seções 3 e 5 em MySQL.");
    assert!(text_of(&r).contains("task_00000000"));
    assert!(text_of(&r).contains("dismiss_task"));
    assert_eq!(
        text_of(&r),
        "Noted (task_00000000). A card is showing in the Maestria panel for this session: the user can start it in a new session, in a fresh worktree or in their checkout, send it to this session, or dismiss it. If it becomes stale or superseded, call dismiss_task with this task_id. Currently pending: task_00000000 \"Converter os exemplos SQL\". Continue your current work."
    );
}

#[tokio::test]
async fn empty_prompt_is_a_tool_error_not_a_protocol_error() {
    let desk = Desk::default();
    let r = handle(
        call("suggest_task", json!({"title": "x", "prompt": " "})),
        "tab1",
        &desk,
    )
    .await
    .unwrap();
    assert_eq!(r["result"]["isError"], true);
    assert!(desk.pending().is_empty());
}

#[tokio::test]
async fn prompt_over_the_cap_is_refused() {
    let prompt = "a".repeat(32001);
    let r = handle(
        call("suggest_task", json!({"title": "x", "prompt": prompt})),
        "tab1",
        &Desk::default(),
    )
    .await
    .unwrap();
    assert_eq!(r["result"]["isError"], true);
    assert!(text_of(&r).starts_with("prompt is too long (32001 chars; max 32000)."));
}

#[tokio::test]
async fn prompt_cap_counts_utf16_units() {
    let prompt = "😀".repeat(16001);
    let r = handle(
        call("suggest_task", json!({"title": "x", "prompt": prompt})),
        "tab1",
        &Desk::default(),
    )
    .await
    .unwrap();
    assert_eq!(r["result"]["isError"], true);
}

#[tokio::test]
async fn without_a_panel_the_session_hears_there_is_nowhere_to_show_it() {
    let r = handle(
        call("suggest_task", json!({"title": "x", "prompt": "y"})),
        "tab9",
        &Desk::default(),
    )
    .await
    .unwrap();
    assert_eq!(r["result"]["isError"], true);
}

#[tokio::test]
async fn withdrawing_once_withdraws_and_the_second_time_says_already_dismissed() {
    let desk = Desk::default();
    handle(
        call("suggest_task", json!({"title": "x", "prompt": "y"})),
        "tab1",
        &desk,
    )
    .await;
    let dismiss = || call("dismiss_task", json!({"task_id": "task_00000000"}));
    let first = handle(dismiss(), "tab1", &desk).await.unwrap();
    assert!(text_of(&first).contains("withdrawn"));
    assert!(desk.pending().is_empty());
    let again = handle(dismiss(), "tab1", &desk).await.unwrap();
    assert!(text_of(&again).contains("already dismissed"));
}

#[tokio::test]
async fn malformed_task_id_is_refused() {
    let r = handle(
        call("dismiss_task", json!({"task_id": "abc"})),
        "tab1",
        &Desk::default(),
    )
    .await
    .unwrap();
    assert_eq!(r["result"]["isError"], true);
}

#[tokio::test]
async fn unknown_method_is_a_json_rpc_error() {
    let r = handle(
        json!({"jsonrpc": "2.0", "id": 3, "method": "resources/list"}),
        "tab1",
        &NoSink,
    )
    .await
    .unwrap();
    assert_eq!(r["error"]["code"], -32601);
    assert_eq!(r["error"]["message"], "method not found: resources/list");
}

#[tokio::test]
async fn unknown_tool_and_non_object_messages_are_protocol_errors() {
    let r = handle(
        json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {}}),
        "tab1",
        &NoSink,
    )
    .await
    .unwrap();
    assert_eq!(
        r["error"],
        json!({"code": -32602, "message": "unknown tool: null"})
    );
    let r = handle(json!(7), "tab1", &NoSink).await.unwrap();
    assert_eq!(
        r,
        json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32600, "message": "invalid request"}})
    );
}

#[tokio::test]
async fn batches_drop_notifications_and_empty_batches_have_no_answer() {
    let batch = json!([
        {"jsonrpc": "2.0", "method": "notifications/initialized"},
        {"jsonrpc": "2.0", "id": "a", "method": "ping"}
    ]);
    let r = handle(batch, "tab1", &NoSink).await.unwrap();
    assert_eq!(r, json!([{"jsonrpc": "2.0", "id": "a", "result": {}}]));
    assert_eq!(handle(json!([]), "tab1", &NoSink).await, None);
}

#[test]
fn task_ids_are_eight_lowercase_hex_digits() {
    for _ in 0..50 {
        assert!(is_task_id(&new_task_id()));
    }
    assert!(!is_task_id("task_ABCDEF12"));
}

#[tokio::test]
async fn the_panel_in_the_url_is_the_panel_that_gets_the_suggestion() {
    let state = tempfile::tempdir().unwrap();
    let (server, _rx) = start(state.path()).await;
    let desk = Arc::new(Desk::default());
    server.set_sink(desk.clone());
    let body = call("suggest_task", json!({"title": "x", "prompt": "y"})).to_string();
    let (status, text) = post(server.port(), "/mcp/tab1", &body).await;
    assert_eq!(status, 200);
    let answer: Value = serde_json::from_str(&text).unwrap();
    assert!(text_of(&answer).contains("Noted"));
    assert_eq!(desk.pending().len(), 1);
}

#[tokio::test]
async fn http_notification_gets_202_without_body_and_get_gets_405() {
    let state = tempfile::tempdir().unwrap();
    let (server, _rx) = start(state.path()).await;
    let body = json!({"jsonrpc": "2.0", "method": "notifications/initialized"}).to_string();
    let (status, text) = post(server.port(), "/mcp/tab1", &body).await;
    assert_eq!((status, text.as_str()), (202, ""));
    let res = reqwest::get(format!("http://127.0.0.1:{}/mcp/tab1", server.port()))
        .await
        .unwrap();
    assert_eq!(res.status().as_u16(), 405);
    assert!(res.text().await.unwrap().is_empty());
    let (status, text) = post(server.port(), "/mcp/tab1", "{nope").await;
    assert_eq!(status, 200);
    assert_eq!(
        text,
        r#"{"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"parse error"}}"#
    );
}

#[tokio::test]
async fn before_a_sink_is_set_the_session_hears_there_is_no_panel() {
    let state = tempfile::tempdir().unwrap();
    let (server, _rx) = start(state.path()).await;
    let body = call("suggest_task", json!({"title": "x", "prompt": "y"})).to_string();
    let (_, text) = post(server.port(), "/mcp/tab1", &body).await;
    let answer: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(answer["result"]["isError"], true);
}

#[tokio::test]
async fn mcp_config_and_permission_point_at_the_same_server() {
    let state = tempfile::tempdir().unwrap();
    let (server, _rx) = start(state.path()).await;
    let port = server.port();
    let config = server.mcp_config_for("tab1");
    assert_eq!(
        config,
        format!(
            r#"{{"mcpServers":{{"maestria":{{"type":"http","url":"http://127.0.0.1:{port}/mcp/tab1","alwaysLoad":true}}}}}}"#
        )
    );
    let settings: Value = serde_json::from_str(&server.settings_for("tab1", None, None)).unwrap();
    assert_eq!(
        settings["permissions"]["allow"],
        json!(["mcp__maestria__suggest_task", "mcp__maestria__dismiss_task"])
    );
}
