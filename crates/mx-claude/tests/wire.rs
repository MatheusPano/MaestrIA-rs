use std::path::Path;

use mx_claude::mcp::{INSTRUCTIONS, tools_json};
use mx_claude::{STATUS_LINE_SCRIPT, mcp_config_for, settings_for};
use serde_json::Value;

fn spec() -> String {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/spec/03-claude-integration.md");
    std::fs::read_to_string(path).unwrap()
}

fn block_after(spec: &str, marker: &str, fence: &str) -> String {
    let from = spec.find(marker).unwrap() + marker.len();
    let rest = &spec[from..];
    let start = rest.find(fence).unwrap() + fence.len() + 1;
    let end = rest[start..].find("\n```").unwrap();
    rest[start..start + end + 1].to_owned()
}

#[test]
fn tools_are_identical_to_the_spec_including_key_order() {
    let block = block_after(&spec(), "`TOOLS` (exact JSON, this key order):", "```json");
    let expected: Value = serde_json::from_str(&block).unwrap();
    assert_eq!(tools_json().to_string(), expected.to_string());
}

#[test]
fn instructions_and_status_line_script_are_identical_to_the_spec() {
    let spec = spec();
    let instructions = block_after(&spec, "`INSTRUCTIONS`:", "```");
    assert_eq!(INSTRUCTIONS, instructions.trim_end_matches('\n'));
    let script = block_after(
        &spec,
        "(a test compares file content to the constant):",
        "```sh",
    );
    assert_eq!(STATUS_LINE_SCRIPT, script);
}

#[test]
fn settings_and_mcp_config_are_byte_exact() {
    let hook =
        r#"[{"hooks":[{"type":"http","url":"http://127.0.0.1:4321/hook/tab1","timeout":5}]}]"#;
    let tool = r#"[{"matcher":"*","hooks":[{"type":"http","url":"http://127.0.0.1:4321/hook/tab1","timeout":5}]}]"#;
    let expected = format!(
        concat!(
            r#"{{"permissions":{{"allow":["mcp__maestria__suggest_task","mcp__maestria__dismiss_task"]}},"#,
            r#""hooks":{{"SessionStart":{h},"SessionEnd":{h},"UserPromptSubmit":{h},"PreToolUse":{t},"#,
            r#""PostToolUse":{t},"PostToolUseFailure":{t},"Notification":{h},"Stop":{h},"StopFailure":{h},"SubagentStop":{h}}},"#,
            r#""statusLine":{{"type":"command","command":"sh '/state/statusline.sh' 'http://127.0.0.1:4321/status/tab1' ''"}}}}"#
        ),
        h = hook,
        t = tool
    );
    assert_eq!(
        settings_for(4321, Path::new("/state/statusline.sh"), "tab1", None, None),
        expected
    );
    assert_eq!(
        mcp_config_for(4321, "tab1"),
        r#"{"mcpServers":{"maestria":{"type":"http","url":"http://127.0.0.1:4321/mcp/tab1","alwaysLoad":true}}}"#
    );
}

#[test]
fn launch_puts_mcp_config_first_and_the_prompt_last() {
    let launch = mx_claude::ClaudeLaunch {
        port: 4321,
        mcp_config: "{\"m\":1}".into(),
        settings: "{\"s\":'x'}".into(),
        name: "repo".into(),
        brief: Some("  briefing  ".into()),
        project: Some("TASK#1".into()),
        resume_id: Some("abc".into()),
        prompt: Some(" faz isso ".into()),
    };
    assert_eq!(
        launch.command_line(),
        r#"claude --mcp-config '{"m":1}' --name 'repo' --settings '{"s":'\''x'\''}' --append-system-prompt 'briefing' --resume 'abc' 'faz isso'"#
    );
    assert_eq!(
        launch.display_line(),
        "claude --mcp-config <tarefas :4321> --name 'repo' --settings <hooks :4321> --append-system-prompt <briefing de TASK#1> --resume … <prompt inicial>"
    );
    let bare = mx_claude::ClaudeLaunch {
        brief: Some("   ".into()),
        resume_id: None,
        prompt: None,
        ..launch
    };
    assert_eq!(
        bare.display_line(),
        "claude --mcp-config <tarefas :4321> --name 'repo' --settings <hooks :4321>"
    );
}
