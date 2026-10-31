use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::time::Duration;

use mx_core::model::TabKind;
use serde_json::{Value, json};

use super::Rig;

const ECHO_CWD: &str = r#"#!/bin/sh
while IFS= read -r l; do
  id=$(echo "$l" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
  case "$l" in
    *'"method":"initialize"'*) echo "{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{}}" ;;
    *'"method":"tool.call"'*)
      cwd=$(echo "$l" | sed -n 's/.*"cwd":"\([^"]*\)".*/\1/p')
      echo "{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{\"content\":[{\"type\":\"text\",\"text\":\"em $cwd\"}]}}" ;;
    *'"method":"shutdown"'*) exit 0 ;;
  esac
done
"#;

fn plugin(root: &str, id: &str, main: &str) {
    let dir = Path::new(root).join(id);
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = json!({"id": id, "version": "1", "main": [main],
        "contributes": {"tools": [{"name": format!("{id}_eco"), "description": "eco", "inputSchema": {"type": "object"}}]}});
    std::fs::write(dir.join(mx_plugins::MANIFEST_NAME), manifest.to_string()).unwrap();
    std::fs::write(dir.join("p.sh"), ECHO_CWD).unwrap();
    std::fs::set_permissions(dir.join("p.sh"), std::fs::Permissions::from_mode(0o755)).unwrap();
}

async fn call(rig: &mut Rig, tools: &std::sync::Arc<dyn mx_claude::mcp::PluginTools>, name: &str) -> Option<Value> {
    let tools = tools.clone();
    let name = name.to_owned();
    let job = tokio::spawn(async move { tools.call("tab1".into(), name, json!({})).await });
    while !job.is_finished() {
        rig.s.pump();
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    job.await.unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn the_bridge_lists_plugin_tools_and_calls_them_from_the_tab_folder() {
    let mut rig = Rig::new();
    let root = rig.mkdir("plugins");
    plugin(&root, "eco", "./p.sh");
    plugin(&root, "quebrado", "./nada.sh");
    rig.add_tab("tab1", "/repo", TabKind::Claude, "/repo/sub", None);
    rig.s.attach_plugins(mx_plugins::Plugins::with_dirs(root.into(), rig.mkdir("plugin-data").into()));
    let plugins = rig.s.plugins().unwrap().clone();
    let tools = rig.s.plugin_tools(plugins.clone());
    let names: Vec<Value> = tools.list().into_iter().map(|t| t["name"].clone()).collect();
    assert_eq!(names, [json!("eco_eco"), json!("quebrado_eco")]);

    let ok = call(&mut rig, &tools, "eco_eco").await.unwrap();
    assert_eq!(ok, json!({"content": [{"type": "text", "text": "em /repo/sub"}]}));
    let failed = call(&mut rig, &tools, "quebrado_eco").await.unwrap();
    assert_eq!(failed["isError"], true);
    assert!(failed["content"][0]["text"].as_str().unwrap().contains("nada.sh"));
    assert_eq!(call(&mut rig, &tools, "eco_nada").await, None);
    plugins.stop_all().await;
}
