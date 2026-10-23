use mx_core::model::{FlowHold, FollowUpKind};

use crate::chrome::flow::model::{self, Draft, Session, Trigger};
use crate::chrome::menu::Tone;

fn session(at_rest: bool, forks: usize, hold: Option<FlowHold>) -> Trigger {
    Trigger::Existing(Session { title: "um".into(), status: "Rodando".into(), tone: Tone::Accent, forks, hold, at_rest })
}

#[test]
fn working_tab_with_a_keep_going_step_arms() {
    let t = session(false, 0, None);
    let steps = vec![Draft { kind: FollowUpKind::KeepGoing, text: "segue".into(), target: None }];
    assert_eq!(model::verb(&t, &steps), "Armar");
    let ready = model::ready(&steps);
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0].kind, FollowUpKind::KeepGoing);
    assert_eq!(ready[0].text, "segue");
}

#[test]
fn tab_at_rest_with_a_command_step_fires() {
    let steps = vec![Draft { kind: FollowUpKind::Command, text: "ls".into(), target: None }];
    assert_eq!(model::verb(&session(true, 0, None), &steps), "Disparar");
    assert_eq!(model::verb(&session(true, 0, None), &[]), "Limpar a fila");
}

#[test]
fn tab_with_two_subagents_shows_agents_and_hold() {
    let Trigger::Existing(s) = session(false, 2, Some(FlowHold::Forks)) else { panic!() };
    let line = model::standing(&s);
    assert!(line.contains("2 agentes rodando"));
    assert!(line.contains("esperando os agentes que ela abriu"));
}

#[test]
fn new_flow_needs_a_prompt_and_new_session_step_is_prefilled_with_review() {
    let t = Trigger::New { place: "meu-repo".into() };
    assert_eq!(model::subtitle(&t, 0), "Em meu-repo");
    assert!(!model::can_save(&t, "  "));
    assert!(model::can_save(&t, "faz"));
    let d = Draft::new(FollowUpKind::NewSession);
    assert!(d.text.contains("git diff"));
    assert_eq!(model::verb(&t, &[d]), "Abrir e armar");
    assert_eq!(model::verb(&t, &[]), "Abrir a sessão");
}

#[test]
fn handoff_is_offered_only_with_targets_and_needs_one() {
    assert!(!model::kinds(false).contains(&FollowUpKind::Handoff));
    assert!(model::kinds(true).contains(&FollowUpKind::Handoff));
    let mut d = Draft::new(FollowUpKind::Handoff);
    assert!(!d.filled());
    d.target = Some("t".into());
    assert!(d.filled());
}

#[test]
fn existing_subtitle_counts_steps() {
    assert_eq!(model::subtitle(&session(false, 0, None), 1), "1 passo depois de \"um\"");
    assert_eq!(model::subtitle(&session(false, 0, None), 2), "2 passos depois de \"um\"");
}
