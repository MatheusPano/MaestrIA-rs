use std::collections::HashSet;
use std::sync::Arc;

use gpui::{App, AppContext, Global, Window};
use mx_core::model::ClaudeStatus;
use mx_sys::git::Git;

use crate::app::ui;
use crate::chrome::dialog::confirm::{self, Confirm, ConfirmDialog, WorktreeSafety};

pub fn ask(c: Confirm, cx: &mut App, on_go: impl Fn(bool, &mut Window, &mut App) + 'static) {
    let generation = ui::upcoming(cx);
    let view = cx.new(|_| {
        ConfirmDialog::new(c, move |answer, window, cx| {
            ui::close_if(generation, cx);
            if let Some(checked) = answer {
                on_go(checked, window, cx);
            }
        })
    });
    ui::modal(view.into(), cx);
}

#[derive(Default)]
struct Probing(HashSet<String>);

impl Global for Probing {}

fn start_probe(path: &str, cx: &mut App) -> bool {
    cx.default_global::<Probing>().0.insert(path.to_owned())
}

fn end_probe(path: &str, cx: &mut App) {
    cx.default_global::<Probing>().0.remove(path);
}

pub fn remove_worktree(root: &str, path: &str, _window: &mut Window, cx: &mut App) {
    let Some(w) = ui::store(cx).worktrees_of(root).iter().find(|w| w.path == path).cloned() else { return };
    if !start_probe(&w.path, cx) {
        return;
    }
    let host = ui::host(cx);
    let rt = host.read(cx).rt();
    let (root, probe) = (root.to_owned(), w.clone());
    let probe_root = root.clone();
    let task = rt.spawn(async move { Git::new(Arc::new(mx_sys::LoginShell)).safety_of(&probe, &probe_root).await });
    cx.spawn(async move |cx| {
        let probed = task.await;
        cx.update(|cx| {
            end_probe(&w.path, cx);
            let Ok(safety) = probed else { return };
            let label = if w.is_main { w.path.clone() } else { w.short_label() };
            let branch = (!w.prunable && !w.is_main && !w.branch.is_empty()).then_some(w.branch.as_str());
            let shown = WorktreeSafety {
                dirty: safety.dirty,
                base: safety.base.clone(),
                unmerged: safety.unmerged as usize,
            };
            let c = confirm::remove_worktree(&label, &w.path, w.prunable, &shown, branch);
            let force = !w.prunable && safety.risky();
            let force_branch = Some(safety.force_branch());
            ask(c, cx, move |delete_branch, _, cx| {
                ui::act(cx, |s| s.remove_worktree(&root, &w, force, force_branch, delete_branch));
            });
        })
        .ok();
    })
    .detach();
}

pub fn close_workspace(id: &str, cx: &mut App) {
    let store = ui::store(cx);
    let Some(w) = store.workspace_by_id(id) else { return };
    let going = store.closing_with(id);
    let sessions = going
        .iter()
        .map(|f| store.tabs_of(&f.root).iter().filter(|t| !t.exited).count())
        .sum();
    let c = confirm::close_workspace(&w.name, w.code_workspace_path.as_deref(), going.len(), sessions);
    let id = id.to_owned();
    ask(c, cx, move |_, _, cx| ui::act(cx, |s| s.close_workspace(&id)));
}

pub fn clear_groups(cx: &mut App) {
    let groups = ui::store(cx).groups().to_vec();
    match groups.as_slice() {
        [] => {}
        [one] => ui::act(cx, |s| {
            s.remove_group(&one.id);
            s.show_banner(&format!("Grupo \"{}\" esquecido — os painéis continuam abertos", one.name));
        }),
        many => ask(confirm::clear_groups(many.len()), cx, |_, _, cx| {
            ui::act(cx, |s| {
                let n = s.clear_groups();
                s.show_banner(&format!("{n} grupos esquecidos — nenhum painel foi fechado"));
            })
        }),
    }
}

pub fn completed_banner(name: &str, closed: usize) -> String {
    match closed {
        0 => format!("\"{name}\" concluído 🎉"),
        n => format!("\"{name}\" concluído 🎉 — {n} painel(is) fechado(s)"),
    }
}

pub fn complete_feature(id: &str, cx: &mut App) {
    let store = ui::store(cx);
    let Some(f) = store.feature_by_id(id).cloned() else { return };
    let tabs = store.tabs_in(id);
    let busy = tabs
        .iter()
        .filter(|t| !t.exited && matches!(t.status(), ClaudeStatus::Working | ClaudeStatus::Tool))
        .count();
    let waiting = tabs.iter().filter(|t| !t.done && t.status().needs_human()).count();
    let c = confirm::complete_feature(&f.name, f.kind, tabs.len(), busy, waiting, !f.brief.trim().is_empty());
    let id = id.to_owned();
    ask(c, cx, move |_, _, cx| {
        ui::act(cx, |s| {
            let closed = s.complete_feature(&id);
            s.show_banner(&completed_banner(&f.name, closed));
        });
        ui::fire_confetti(cx);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[gpui::test]
    fn a_second_delete_click_during_the_git_probe_is_ignored(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| {
            assert!(start_probe("/wt", cx));
            assert!(!start_probe("/wt", cx));
            end_probe("/wt", cx);
            assert!(start_probe("/wt", cx));
        });
    }

    #[test]
    fn completing_names_the_closed_panes() {
        assert_eq!(completed_banner("Login", 0), "\"Login\" concluído 🎉");
        assert_eq!(completed_banner("Login", 2), "\"Login\" concluído 🎉 — 2 painel(is) fechado(s)");
    }
}
