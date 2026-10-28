use mx_core::model::TabKind;

use super::Rig;
use crate::ClaudeSpec;

fn focused_claude(rig: &mut Rig) -> String {
    let folder = rig.s1();
    let id = rig.s.open_claude(ClaudeSpec::new(&folder, "/repo"));
    rig.s.focus_pane(&id);
    id
}

fn assert_alive_in_background(rig: &Rig, claude: &str) {
    assert!(rig.s.has_tab(claude), "claude tab was dropped");
    assert!(!rig.s.is_open(claude));
    assert_eq!(rig.terms.log(claude).kills, 0);
    assert_eq!(rig.terms.log(claude).hang_ups, 0);
    assert!(!rig.t(claude).exited);
}

#[test]
fn a_new_terminal_takes_the_pane_and_keeps_the_claude_tab_alive() {
    let mut rig = Rig::new();
    let claude = focused_claude(&mut rig);
    let folder = rig.s.focused_folder().clone();
    let shell = rig.s.open_shell(&folder, None, None, None, None);
    assert_eq!(rig.s.order(), std::slice::from_ref(&shell));
    assert_eq!(rig.t(&shell).kind, TabKind::Shell);
    assert_alive_in_background(&rig, &claude);
    rig.s.select(&claude);
    assert_eq!(rig.s.order(), [claude]);
}

#[test]
fn another_claude_takes_the_pane_and_keeps_the_first_alive() {
    let mut rig = Rig::new();
    let first = focused_claude(&mut rig);
    let folder = rig.s.focused_folder().clone();
    let second = rig.s.open_claude(ClaudeSpec::new(&folder, "/repo"));
    assert_eq!(rig.s.order(), [second]);
    assert_alive_in_background(&rig, &first);
}

#[test]
fn closing_the_new_terminal_keeps_the_claude_tab() {
    let mut rig = Rig::new();
    let claude = focused_claude(&mut rig);
    let folder = rig.s.focused_folder().clone();
    let shell = rig.s.open_shell(&folder, None, None, None, None);
    rig.s.close_tab(&shell);
    assert!(rig.s.has_tab(&claude));
    assert_eq!(rig.terms.log(&claude).kills, 0);
}
