use gpui::{AnyView, App, AppContext, Window};
use mx_core::model::{FeatureOrHotfixKind, LauncherIcon};

use crate::chrome::dialog::confirm::{self, ConfirmDialog, WorktreeSafety};
use crate::chrome::dialog::form::FormDialog;
use crate::chrome::dialog::lists::{ChatHistoryDialog, ChatRow, ChatSection, More, MoveOption, MoveToDialog};
use crate::chrome::dialog::specs;
use crate::chrome::menu::Tone;
use std::rc::Rc;

pub fn scene(component: &str, window: &mut Window, cx: &mut App) -> AnyView {
    let log = |r| log::info!("dialog result {r:?}");
    let form = |spec, window: &mut Window, cx: &mut App| -> AnyView {
        cx.new(|cx| FormDialog::new(spec, move |r, _, _| log(r), window, cx)).into()
    };
    let confirm = |c, cx: &mut App| -> AnyView { cx.new(|_| ConfirmDialog::new(c, |r, _, _| log::info!("confirm {r:?}"))).into() };
    match component {
        "dialog-prompt" => form(specs::prompt_text("Renomear feature", "Nome", "Permissão do google"), window, cx),
        "dialog-add-folder" => form(specs::add_folder(), window, cx),
        "dialog-loose" => form(specs::loose_in("~/avulsos"), window, cx),
        "dialog-task" => form(specs::new_task("maestria"), window, cx),
        "dialog-workspace" => form(specs::new_workspace(&[("maestria".into(), true), ("lms".into(), false), ("wiboor".into(), true)]), window, cx),
        "dialog-feature" => form(specs::new_feature(FeatureOrHotfixKind::Feature, "maestria"), window, cx),
        "dialog-brief" => form(specs::brief("Permissão do google", "Não mexer no login antigo.\nUsar o client novo."), window, cx),
        "dialog-launcher" => form(specs::launcher_form(Some(("btop", "btop --utf-force", LauncherIcon::Monitor))), window, cx),
        "dialog-worktree" => confirm(
            confirm::remove_worktree("TASK#47730", "/home/lucas/repos/maestria/TASK-47730", false, &WorktreeSafety { dirty: Some(2), base: Some("origin/main".into()), unmerged: 1 }, Some("feature/TASK#47730")),
            cx,
        ),
        "dialog-close-workspace" => confirm(confirm::close_workspace("ATRIUM", Some("~/repos/atrium.code-workspace"), 3, 2), cx),
        "dialog-groups" => confirm(confirm::clear_groups(2), cx),
        "dialog-complete" => confirm(confirm::complete_feature("Permissão do google", FeatureOrHotfixKind::Feature, 3, 1, 1, true), cx),
        "dialog-move" => cx.new(|_| Holder(Box::new(|| MoveToDialog {
            title: crate::chrome::dialog::lists::move_title("Revisar o PR"),
            options: vec![
                MoveOption { id: "f1".into(), name: "Permissão do google".into(), kind: FeatureOrHotfixKind::Feature, current: true },
                MoveOption { id: "f2".into(), name: "Login quebrado".into(), kind: FeatureOrHotfixKind::Hotfix, current: false },
            ],
            loose: false,
            on_pick: Rc::new(|id, _, _| log::info!("move {id:?}")),
            on_cancel: Rc::new(|_, _| {}),
        }.into_any()))).into(),
        _ => cx.new(|_| Holder(Box::new(|| ChatHistoryDialog {
            loading: false,
            sections: vec![
                ChatSection {
                    name: "maestria".into(),
                    rows: vec![
                        ChatRow { title: "Reescrever o chrome em gpui".into(), meta: "2h · maestria-rs · 1,2 MB".into(), note: Some(("na tela".into(), Tone::Green)), gone: false },
                        ChatRow { title: "Plugin do ArgoCD".into(), meta: "1d · argocd-plugin · 340 KB".into(), note: None, gone: false },
                        ChatRow { title: "Worktree apagada".into(), meta: "5d · TASK-1 · 80 KB".into(), note: Some(("Não sei onde ela rodou".into(), Tone::Yellow)), gone: true },
                    ],
                    more: Some(More::Offer("Todas as conversas em maestria".into())),
                },
                ChatSection { name: "avulsos".into(), rows: vec![ChatRow { title: "Ajuda com regex".into(), meta: "3d · ~ · 12 KB".into(), note: None, gone: false }], more: None },
            ],
            on_resume: Rc::new(|p, _, _| log::info!("resume {p:?}")),
            on_more: Rc::new(|i, _, _| log::info!("more {i}")),
            on_close: Rc::new(|_, _| {}),
        }.into_any()))).into(),
    }
}

pub struct Holder(pub Box<dyn Fn() -> gpui::AnyElement>);

impl gpui::Render for Holder {
    fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl gpui::IntoElement {
        (self.0)()
    }
}

trait IntoAny {
    fn into_any(self) -> gpui::AnyElement;
}

impl<E: gpui::IntoElement> IntoAny for E {
    fn into_any(self) -> gpui::AnyElement {
        self.into_any_element()
    }
}
