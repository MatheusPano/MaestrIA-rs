use std::time::Duration;

use serde_json::{Value, json};

use super::Rig;

const LIVE: &str = "MX_LIVE_POMODORO";

async fn until(rig: &mut Rig, what: impl Fn(&Rig) -> bool) -> bool {
    for _ in 0..400 {
        rig.s.pump();
        if what(rig) {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    false
}

fn float_data(rig: &Rig, key: &str) -> Option<Value> {
    let f = rig.s.plugin_ui().floats.shown().iter().find(|f| f.key() == "maestria.pomodoro/timer")?;
    f.view.rfw_data.get(key).cloned()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_real_plugin_shows_its_float_and_answers_a_view_action() {
    let Ok(src) = std::env::var(LIVE) else { return };
    let mut rig = Rig::new();
    let root = rig.mkdir("plugins");
    let data = rig.mkdir("plugin-data");
    let copied = std::process::Command::new("cp").args(["-R", &src, &format!("{root}/pomodoro")]).status().unwrap();
    assert!(copied.success());
    rig.s.attach_plugins(mx_plugins::Plugins::with_dirs(root.into(), data.into()));
    rig.s.start_plugins();
    assert!(until(&mut rig, |r| float_data(r, "clock").is_some()).await, "float never showed");
    let before = float_data(&rig, "running");
    let tab = rig.s.plugin_ui().floats.shown()[0].tab_id.clone();
    let values = json!({"a": "alternar"}).as_object().unwrap().clone();
    rig.s.view_action(&tab, "press", values);
    assert!(until(&mut rig, |r| float_data(r, "running") != before).await, "the action never came back");
    rig.s.show_sidebar_view(Some("maestria.pomodoro"));
    assert!(until(&mut rig, |r| r.s.sidebar_view_of("maestria.pomodoro").is_some()).await, "sidebar never drew");
    rig.s.shutdown().await;
}
