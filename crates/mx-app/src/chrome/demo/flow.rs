use gpui::{AnyView, App, AppContext, Window};
use mx_core::model::{FlowHold, FollowUpKind};

use crate::chrome::flow::{Draft, FlowEditor, Session, Trigger};
use crate::chrome::menu::Tone;

pub fn scene(component: &str, window: &mut Window, cx: &mut App) -> AnyView {
    let (trigger, queued) = if component == "flow-existing" {
        (
            Trigger::Existing(Session { title: "Revisar o PR".into(), status: "Rodando".into(), tone: Tone::Accent, forks: 2, hold: Some(FlowHold::Forks), at_rest: false }),
            vec![Draft { kind: FollowUpKind::KeepGoing, text: "Agora rode os testes e corrija o que quebrar".into(), target: None }, Draft::new(FollowUpKind::NewSession)],
        )
    } else {
        (Trigger::New { place: "maestria".into() }, vec![Draft { kind: FollowUpKind::Command, text: "cargo test -p mx-app".into(), target: None }])
    };
    cx.new(|cx| FlowEditor::new(trigger, queued, vec![("t2".into(), "Lint".into())], |r, _, _| log::info!("flow {r:?}"), window, cx)).into()
}
