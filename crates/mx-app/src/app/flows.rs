use gpui::{App, AppContext, Window};
use mx_core::model::{ClaudeStatus, Folder, TabKind};
use mx_store::Store;

use crate::app::menus::{Place, folder_or_loose};
use crate::app::ui;
use crate::chrome::flow::{Draft, FlowEditor, FlowResult, Session, Trigger};
use crate::chrome::menu::Tone;

pub fn status_tone(s: ClaudeStatus) -> Tone {
    match s {
        ClaudeStatus::Working | ClaudeStatus::Tool => Tone::Accent,
        ClaudeStatus::WaitingInput | ClaudeStatus::WaitingAnswer => Tone::Yellow,
        ClaudeStatus::WaitingPermission => Tone::Red,
        ClaudeStatus::Idle | ClaudeStatus::Ready => Tone::Green,
        ClaudeStatus::Starting => Tone::Purple,
        ClaudeStatus::Unknown | ClaudeStatus::Ended => Tone::Faint,
    }
}

pub fn where_of(folder: &Folder, cwd: &str) -> String {
    let root = &folder.root;
    if cwd == root || !cwd.starts_with(root.as_str()) {
        return folder.name.clone();
    }
    format!("{}/{}", folder.name, cwd.rsplit('/').next().unwrap_or_default())
}

fn targets(store: &Store, root: &str, except: Option<&str>) -> Vec<(String, String)> {
    store
        .tabs()
        .iter()
        .filter(|t| Some(t.id.as_str()) != except && t.kind == TabKind::Claude && !t.exited && t.folder_root == root)
        .map(|t| (t.id.clone(), store.title(t)))
        .collect()
}

fn open(
    trigger: Trigger,
    queued: Vec<Draft>,
    targets: Vec<(String, String)>,
    on_save: impl Fn(FlowResult, &mut App) + 'static,
    window: &mut Window,
    cx: &mut App,
) {
    let generation = ui::upcoming(cx);
    let view = cx.new(|cx| {
        FlowEditor::new(
            trigger,
            queued,
            targets,
            move |result, _, cx| {
                ui::close_if(generation, cx);
                if let Some(r) = result {
                    on_save(r, cx);
                }
            },
            window,
            cx,
        )
    });
    ui::modal(view.into(), cx);
}

pub fn new_flow(place: Place, window: &mut Window, cx: &mut App) {
    let store = ui::store(cx);
    let Some(folder) = folder_or_loose(store, &place.root) else { return };
    let cwd = place.cwd.clone().unwrap_or_else(|| folder.root.clone());
    let trigger = Trigger::New { place: where_of(&folder, &cwd) };
    let targets = targets(store, &folder.root, None);
    open(trigger, Vec::new(), targets, move |r, cx| {
        let label = Some(r.label.as_str()).filter(|l| !l.is_empty());
        ui::act(cx, |s| {
            s.start_flow(&folder, &r.prompt, r.steps.clone(), Some(&cwd), label, place.feature.as_deref());
        });
    }, window, cx);
}

pub fn existing_flow(id: &str, window: &mut Window, cx: &mut App) {
    let store = ui::store(cx);
    let Some(t) = store.tab(id) else { return };
    let status = t.status();
    let hold = store.hold_for(id);
    let session = Session {
        title: store.title(t),
        status: status.label().to_owned(),
        tone: status_tone(status),
        forks: t.hooks.forks_out,
        hold: t.armed.then_some(hold),
        at_rest: status.at_rest(),
    };
    let queued = t
        .follow_ups
        .iter()
        .map(|f| Draft { kind: f.kind, text: f.text.clone(), target: f.target_tab_id.clone() })
        .collect();
    let targets = targets(store, &t.folder_root, Some(id));
    let id = id.to_owned();
    open(Trigger::Existing(session), queued, targets, move |r, cx| {
        ui::act(cx, |s| {
            let now = s.tab(&id).is_some_and(|t| t.status().at_rest());
            s.queue(&id, r.steps.clone(), now);
        });
    }, window, cx);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_worktree_cwd_reads_as_folder_slash_leaf() {
        let f = Folder::new("/r/app", "app");
        assert_eq!(where_of(&f, "/r/app"), "app");
        assert_eq!(where_of(&f, "/r/app/.wt/TASK-1"), "app/TASK-1");
        assert_eq!(where_of(&f, "/elsewhere/TASK-1"), "app");
    }
}
