use std::rc::Rc;

use gpui::{
    AnyElement, App, AppContext, Context, Entity, IntoElement, PathPromptOptions, Render, WeakEntity, Window,
};
use mx_core::model::FeatureOrHotfixKind;
use mx_store::{DocPlace, NewTask};

use crate::app::menus::{Place, folder_or_loose};
use crate::app::ui;
use crate::chrome::dialog::form::{FormDialog, FormResult, FormSpec};
use crate::chrome::dialog::lists::{MoveOption, MoveToDialog, move_title, no_features_banner};
use crate::chrome::dialog::specs;
use crate::chrome::menu::Tone;

pub const PICKER_FAILED: &str = "Não consegui abrir o seletor de arquivos";

pub enum Picked {
    Path(String),
    Cancelled,
    Unavailable,
}

pub fn pick(directories: bool, prompt: &str, on: impl FnOnce(Picked, &mut App) + 'static, cx: &mut App) {
    let rx = cx.prompt_for_paths(PathPromptOptions {
        files: !directories,
        directories,
        multiple: false,
        prompt: Some(prompt.to_owned().into()),
    });
    cx.spawn(async move |cx| {
        let picked = match rx.await {
            Ok(Ok(Some(paths))) => paths
                .first()
                .map_or(Picked::Cancelled, |p| Picked::Path(p.to_string_lossy().into_owned())),
            Ok(Ok(None)) => Picked::Cancelled,
            Ok(Err(err)) => {
                log::warn!("file picker failed: {err:#}");
                Picked::Unavailable
            }
            Err(_) => Picked::Unavailable,
        };
        cx.update(|cx| on(picked, cx)).ok();
    })
    .detach();
}

fn picked_or_banner(on: impl FnOnce(String, &mut App) + 'static) -> impl FnOnce(Picked, &mut App) + 'static {
    move |picked, cx| match picked {
        Picked::Path(p) => on(p, cx),
        Picked::Cancelled => {}
        Picked::Unavailable => ui::act(cx, |s| s.show_banner_sticky(PICKER_FAILED)),
    }
}

pub fn pick_code_workspace(on: impl FnOnce(String, &mut App) + 'static, cx: &mut App) {
    pick(false, "Adicionar", picked_or_banner(on), cx);
}

pub fn pick_markdown(place: DocPlace, _start: &str, cx: &mut App) {
    pick(false, "Abrir", picked_or_banner(move |path, cx| drop(ui::act(cx, |s| s.show_file(&path, place)))), cx);
}

pub fn form(
    spec: FormSpec,
    window: &mut Window,
    cx: &mut App,
    on_ok: impl Fn(FormResult, &mut Window, &mut App) + 'static,
) -> Entity<FormDialog> {
    form_with(spec, window, cx, None, on_ok)
}

type Extra = Box<dyn Fn(usize, WeakEntity<FormDialog>, &mut Window, &mut App)>;

fn form_with(
    spec: FormSpec,
    window: &mut Window,
    cx: &mut App,
    extra: Option<Extra>,
    on_ok: impl Fn(FormResult, &mut Window, &mut App) + 'static,
) -> Entity<FormDialog> {
    let generation = ui::upcoming(cx);
    let view = cx.new(|cx| {
        let dialog = FormDialog::new(
            spec,
            move |result, window, cx| {
                ui::close_if(generation, cx);
                if let Some(r) = result {
                    on_ok(r, window, cx);
                }
            },
            window,
            cx,
        );
        match extra {
            Some(f) => dialog.on_extra(f),
            None => dialog,
        }
    });
    ui::modal(view.clone().into(), cx);
    view
}

fn field(r: &FormResult, i: usize) -> String {
    r.fields.get(i).map(|s| s.trim().to_owned()).unwrap_or_default()
}

fn filled(r: &FormResult, i: usize) -> Option<String> {
    Some(field(r, i)).filter(|s| !s.is_empty())
}

pub fn prompt(
    title: &str,
    label: &str,
    initial: &str,
    on_ok: impl Fn(String, &mut App) + 'static,
    window: &mut Window,
    cx: &mut App,
) {
    form(specs::prompt_text(title, label, initial), window, cx, move |r, _, cx| {
        on_ok(r.fields.first().cloned().unwrap_or_default(), cx)
    });
}

fn fill_from_picker(dialog: WeakEntity<FormDialog>, directories: bool, prompt: &str, cx: &mut App) {
    pick(directories, prompt, move |picked, cx| {
        dialog
            .update(cx, |d, cx| match picked {
                Picked::Path(p) => d.set_field(0, p, cx),
                Picked::Cancelled => {}
                Picked::Unavailable => d.set_note(specs::ADD_FOLDER_TROUBLE, Tone::Yellow, cx),
            })
            .ok();
    }, cx);
}

pub fn add_folder(window: &mut Window, cx: &mut App) {
    let extra: Extra = Box::new(|i, dialog, _, cx| fill_from_picker(dialog, i == 0, if i == 0 { "Escolher" } else { "Adicionar" }, cx));
    form_with(specs::add_folder(), window, cx, Some(extra), |r, _, cx| {
        let path = field(&r, 0);
        if path.is_empty() {
            return;
        }
        if path.to_lowercase().ends_with(".code-workspace") {
            ui::act(cx, |s| s.import_workspace(&path));
        } else {
            ui::act(cx, |s| s.add_folder(&path));
        }
    });
}

pub fn new_workspace(preselect: Option<String>, window: &mut Window, cx: &mut App) {
    let folders: Vec<(String, String)> = ui::store(cx).folders().iter().map(|f| (f.root.clone(), f.name.clone())).collect();
    let checks: Vec<(String, bool)> = folders
        .iter()
        .map(|(root, name)| (name.clone(), preselect.as_deref() == Some(root.as_str())))
        .collect();
    form(specs::new_workspace(&checks), window, cx, move |r, _, cx| {
        let name = field(&r, 0);
        let roots: Vec<&str> = folders
            .iter()
            .zip(&r.checks)
            .filter(|(_, on)| **on)
            .map(|((root, _), _)| root.as_str())
            .collect();
        ui::act(cx, |s| drop(s.create_workspace(&name, &roots, None)));
    });
}

pub fn new_feature(root: &str, kind: FeatureOrHotfixKind, window: &mut Window, cx: &mut App) {
    let Some(folder) = folder_or_loose(ui::store(cx), root) else { return };
    let root = folder.root.clone();
    form(specs::new_feature(kind, &folder.name), window, cx, move |r, _, cx| {
        let name = field(&r, 0);
        let brief = r.fields.get(1).cloned().unwrap_or_default();
        ui::act(cx, |s| drop(s.add_feature(&root, &name, brief.trim(), kind)));
    });
}

pub fn brief(id: &str, window: &mut Window, cx: &mut App) {
    let Some(f) = ui::store(cx).feature_by_id(id).cloned() else { return };
    let id = id.to_owned();
    form(specs::brief(&f.name, &f.brief), window, cx, move |r, _, cx| {
        let text = r.fields.first().cloned().unwrap_or_default();
        ui::act(cx, |s| s.edit_feature_text(&id, None, Some(text.trim())));
    });
}

pub fn new_task(root: &str, feature: Option<String>, window: &mut Window, cx: &mut App) {
    let store = ui::store(cx);
    let place = feature
        .as_deref()
        .and_then(|id| store.feature_by_id(id))
        .map(|f| f.name.clone())
        .or_else(|| store.find_folder(root).map(|f| f.name.clone()))
        .unwrap_or_default();
    let root = root.to_owned();
    form(specs::new_task(&place), window, cx, move |r, _, cx| {
        let Some(task_id) = filled(&r, 0) else { return };
        let task = NewTask {
            folder_root: root.clone(),
            task_id,
            branch_pattern: field(&r, 1),
            dir_pattern: field(&r, 2),
            base_ref: filled(&r, 3),
            setup_command: filled(&r, 4),
            feature: feature.clone(),
            prompt: None,
            select: true,
            reply: None,
        };
        ui::act(cx, |s| s.new_task(task));
    });
}

pub fn launcher_form(editing: Option<String>, then_open: Option<Place>, window: &mut Window, cx: &mut App) {
    let current = editing.as_deref().and_then(|id| ui::store(cx).launcher_by_id(id)).cloned();
    let spec = specs::launcher_form(current.as_ref().map(|l| (l.name.as_str(), l.command.as_str(), l.icon)));
    form(spec, window, cx, move |r, _, cx| {
        let command = field(&r, 0);
        let name = specs::launcher_name(&command, &field(&r, 1));
        let icon = specs::LAUNCHER_ICONS[r.chip.unwrap_or(0).min(specs::LAUNCHER_ICONS.len() - 1)];
        match &editing {
            Some(id) => ui::act(cx, |s| s.edit_launcher(id, Some(&name), Some(&command), Some(icon))),
            None => {
                let place = then_open.clone();
                ui::act(cx, |s| {
                    let id = s.add_launcher(&name, &command, icon);
                    let Some(place) = place else { return };
                    let Some(folder) = folder_or_loose(s, &place.root) else { return };
                    let cwd = place.cwd.clone().unwrap_or_else(|| folder.root.clone());
                    s.open_launcher(&id, &folder, Some(&cwd), place.feature.as_deref());
                });
            }
        }
    });
}

pub struct Shown(Box<dyn Fn() -> AnyElement>);

impl Render for Shown {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        (self.0)()
    }
}

pub fn shown(cx: &mut App, f: impl Fn() -> AnyElement + 'static) -> u64 {
    let view = cx.new(|_| Shown(Box::new(f)));
    ui::modal(view.into(), cx)
}

pub fn move_to(tab: &str, cx: &mut App) {
    let store = ui::store(cx);
    let Some(t) = store.tab(tab) else { return };
    let loose = t.loose;
    let options: Vec<MoveOption> = store
        .features_of(&t.folder_root)
        .into_iter()
        .map(|f| MoveOption {
            id: f.id.clone(),
            name: f.name.clone(),
            kind: f.kind,
            current: t.feature_or_hotfix_id.as_deref() == Some(f.id.as_str()),
        })
        .collect();
    if options.is_empty() {
        ui::act(cx, |s| s.show_banner(no_features_banner(loose)));
        return;
    }
    let title = move_title(&store.title(t));
    let tab = tab.to_owned();
    let generation = ui::upcoming(cx);
    let pick = Rc::new(move |id: Option<String>, _: &mut Window, cx: &mut App| {
        ui::close_if(generation, cx);
        ui::act(cx, |s| s.assign(&tab, id.as_deref()));
    });
    shown(cx, move || {
        MoveToDialog {
            title: title.clone(),
            options: options.clone(),
            loose,
            on_pick: pick.clone(),
            on_cancel: Rc::new(move |_, cx| ui::close_if(generation, cx)),
        }
        .into_any_element()
    });
}
