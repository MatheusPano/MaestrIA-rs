use std::cell::RefCell;
use std::rc::Rc;

use gpui::{App, ClipboardItem, Pixels, Point, WeakEntity, Window};
use mx_store::{ClaudeSpec, DocPlace, Store};

use crate::app::menus::{self, Place, Target, core_tint, folder_or_loose};
use crate::app::{confirms, dialogs, flows, ui};
use crate::chrome::menu::{ContextMenu, MenuAction as A, MenuEntry};

pub fn open_here(place: &Place, action: &A, window: &mut Window, cx: &mut App) -> bool {
    let Some(folder) = folder_or_loose(ui::store(cx), &place.root) else { return false };
    let cwd = place.cwd.clone().unwrap_or_else(|| folder.root.clone());
    let feature = place.feature.as_deref();
    match action {
        A::OpenClaude => {
            let mut spec = ClaudeSpec::new(&folder, &cwd);
            spec.label = place.label.clone();
            spec.feature = place.feature.clone();
            ui::act(cx, |s| s.open_claude(spec));
        }
        A::OpenShell => {
            ui::act(cx, |s| s.open_shell(&folder, place.cwd.as_deref(), None, feature, None));
        }
        A::OpenLauncher(id) => {
            ui::act(cx, |s| s.open_launcher(id, &folder, Some(&cwd), feature));
        }
        A::NewFlow => flows::new_flow(place.clone(), window, cx),
        A::NewLauncher => dialogs::launcher_form(None, Some(place.clone()), window, cx),
        A::OpenMarkdown => {
            let doc = DocPlace {
                from: None,
                folder: Some(folder.root.clone()),
                cwd: place.cwd.clone(),
                feature: place.feature.clone(),
            };
            dialogs::pick_markdown(doc, &cwd, cx);
        }
        _ => return false,
    }
    true
}

fn place_of(store: &Store, target: &Target) -> Option<Place> {
    Some(match target {
        Target::Folder { root, .. } => Place::at(root),
        Target::Plus { root, feature } => Place { feature: feature.clone(), ..Place::at(root) },
        Target::Feature(id) => {
            let f = store.feature_by_id(id)?;
            Place { feature: Some(f.id.clone()), ..Place::at(&f.folder_root) }
        }
        Target::Worktree { root, path } => {
            let w = store.worktrees_of(root).iter().find(|w| &w.path == path)?;
            Place {
                root: root.clone(),
                cwd: Some(path.clone()),
                feature: None,
                label: (!w.is_main).then(|| w.short_label()),
            }
        }
        _ => return None,
    })
}

pub fn run(target: &Target, action: A, window: &mut Window, cx: &mut App) {
    if let Some(place) = place_of(ui::store(cx), target)
        && open_here(&place, &action, window, cx)
    {
        return;
    }
    match target {
        Target::Folder { root, within } => folder(root, within.as_deref(), action, window, cx),
        Target::Plus { root, .. } | Target::Worktrees(root) if matches!(action, A::NewFeatureOrHotfix(_) | A::PruneWorktrees | A::Worktree(_)) => {
            match action {
                A::NewFeatureOrHotfix(kind) => dialogs::new_feature(root, kind, window, cx),
                A::PruneWorktrees => ui::act(cx, |s| s.prune_worktrees(root)),
                A::Worktree(path) => {
                    let at = menus::nudge(window.mouse_position(), 28., 12.);
                    menus::open_at(Target::Worktree { root: root.clone(), path }, at, cx);
                }
                _ => {}
            }
        }
        Target::Worktree { root, path } => worktree(root, path, action, window, cx),
        Target::Workspace(id) => workspace(id, action, window, cx),
        Target::Feature(id) => feature(id, action, window, cx),
        Target::Loose => {
            let root = ui::store(cx).loose().root.clone();
            match action {
                A::NewFeatureOrHotfix(kind) => dialogs::new_feature(&root, kind, window, cx),
                A::CloseSettled => drop(ui::act(cx, |s| s.close_settled(&root))),
                _ => {}
            }
        }
        Target::Group(id) => group(id, action, window, cx),
        Target::Panel(id) => crate::app::panel_actions::run(id, action, window, cx),
        Target::FooterAdd => match action {
            A::AddFolder => dialogs::add_folder(window, cx),
            A::ImportWorkspace => dialogs::pick_code_workspace(|path, cx| ui::act(cx, |s| s.import_workspace(&path)), cx),
            A::NewWorkspace => dialogs::new_workspace(None, window, cx),
            _ => {}
        },
        _ => {}
    }
}

fn folder(root: &str, within: Option<&str>, action: A, window: &mut Window, cx: &mut App) {
    let root = root.to_owned();
    match action {
        A::Setup => drop(ui::act(cx, |s| s.show_setup(&root, None, None))),
        A::Worktrees => {
            let at = menus::nudge(window.mouse_position(), 14., 6.);
            menus::open_at(Target::Worktrees(root), at, cx);
        }
        A::NewFeatureOrHotfix(kind) => dialogs::new_feature(&root, kind, window, cx),
        A::Rename => {
            let name = ui::store(cx).find_folder(&root).map(|f| f.name.clone()).unwrap_or_default();
            dialogs::prompt("Renomear pasta", "Nome", &name, move |text, cx| ui::act(cx, |s| s.rename_folder(&root, &text)), window, cx);
        }
        A::Tint(t) => ui::act(cx, |s| s.set_folder_tint(&root, core_tint(t))),
        A::ToggleWorkspace(ws) => ui::act(cx, |s| {
            let member = s.workspace_by_id(&ws).is_some_and(|w| w.folder_roots.contains(&root));
            if member {
                s.remove_from_workspace(&root, &ws, None);
            } else {
                s.add_to_workspace(&root, &ws, None);
            }
        }),
        A::NewWorkspace => dialogs::new_workspace(Some(root), window, cx),
        A::LeaveWorkspace => {
            if let Some(ws) = within {
                ui::act(cx, |s| s.remove_from_workspace(&root, ws, None));
            }
        }
        A::CloseSettled => drop(ui::act(cx, |s| s.close_settled(&root))),
        A::RemoveFolder => ui::act(cx, |s| s.remove_folder(&root)),
        _ => {}
    }
}

fn worktree(root: &str, path: &str, action: A, window: &mut Window, cx: &mut App) {
    match action {
        A::OpenInEditor => ui::act(cx, |s| s.open_in_editor(path)),
        A::Setup => drop(ui::act(cx, |s| s.show_setup(root, Some(path), None))),
        A::CopyPath => {
            cx.write_to_clipboard(ClipboardItem::new_string(path.to_owned()));
            ui::act(cx, |s| s.show_banner("Caminho copiado"));
        }
        A::RemoveWorktree => confirms::remove_worktree(root, path, window, cx),
        _ => {}
    }
}

fn workspace(id: &str, action: A, window: &mut Window, cx: &mut App) {
    let id = id.to_owned();
    let Some(w) = ui::store(cx).workspace_by_id(&id).cloned() else { return };
    match action {
        A::Rename => dialogs::prompt("Renomear workspace", "Nome", &w.name, move |text, cx| ui::act(cx, |s| s.rename_workspace(&id, &text)), window, cx),
        A::Tint(t) => ui::act(cx, |s| s.set_workspace_tint(&id, core_tint(t))),
        A::OpenInEditor => {
            if let Some(path) = &w.code_workspace_path {
                ui::act(cx, |s| s.open_in_editor(path));
            }
        }
        A::UnlinkCodeWorkspace => ui::act(cx, |s| s.link_code_workspace(&id, None)),
        A::LinkCodeWorkspace => dialogs::pick_code_workspace(move |path, cx| ui::act(cx, |s| s.link_code_workspace(&id, Some(&path))), cx),
        A::DissolveWorkspace => ui::act(cx, |s| {
            s.dissolve_workspace(&id);
            s.show_banner(&format!("Workspace \"{}\" desfeito — as pastas continuam na lateral", w.name));
        }),
        A::CloseWorkspace => confirms::close_workspace(&id, cx),
        _ => {}
    }
}

fn feature(id: &str, action: A, window: &mut Window, cx: &mut App) {
    let id = id.to_owned();
    let Some(f) = ui::store(cx).feature_by_id(&id).cloned() else { return };
    match action {
        A::Setup => drop(ui::act(cx, |s| s.show_setup(&f.folder_root, None, Some(&id)))),
        A::NewTask => dialogs::new_task(&f.folder_root, Some(id), window, cx),
        A::Brief => dialogs::brief(&id, window, cx),
        A::Rename => {
            let title = format!("Renomear {}", f.kind.label());
            dialogs::prompt(&title, "Nome", &f.name, move |text, cx| ui::act(cx, |s| s.edit_feature_text(&id, Some(&text), None)), window, cx);
        }
        A::Tint(t) => ui::act(cx, |s| s.set_feature_tint(&id, core_tint(t))),
        A::SwitchKind => ui::act(cx, |s| s.set_feature_kind(&id, f.kind.other())),
        A::Complete => confirms::complete_feature(&id, cx),
        A::Dissolve => ui::act(cx, |s| {
            let loose = f.folder_root == s.loose().root;
            s.remove_feature(&id);
            let where_ = if loose { "nos avulsos" } else { "na pasta" };
            s.show_banner(&format!("{} — os painéis continuam abertos {where_}", f.kind.dissolved()));
        }),
        _ => {}
    }
}

fn group(id: &str, action: A, window: &mut Window, cx: &mut App) {
    let id = id.to_owned();
    let Some(g) = ui::store(cx).group_by_id(&id).cloned() else { return };
    match action {
        A::SaveGroup => ui::act(cx, |s| {
            let text = match s.save_group(&g.name).and_then(|gid| s.group_by_id(&gid)) {
                Some(saved) => format!("Grupo \"{}\" agora é {}", saved.name, saved.summary()),
                None => format!("Não há nada na tela pra guardar no grupo \"{}\"", g.name),
            };
            s.show_banner(&text);
        }),
        A::Rename => dialogs::prompt("Renomear grupo", "Nome", &g.name, move |text, cx| ui::act(cx, |s| s.rename_group(&id, &text)), window, cx),
        A::ForgetGroup => ui::act(cx, |s| s.remove_group(&id)),
        _ => {}
    }
}

pub fn filter_menu(entries: Vec<MenuEntry<A>>, at: Point<Pixels>, cx: &mut App) {
    let cell: Rc<RefCell<Option<WeakEntity<ContextMenu<A>>>>> = Rc::default();
    let me = cell.clone();
    let menu = ui::menu(
        entries,
        at,
        move |action, _, cx| {
            ui::act(cx, |s| match action {
                A::FilterFolder(root) => s.toggle_filter_folder(&root),
                A::FilterFeature(id) => s.toggle_filter_feature(&id),
                A::Filter(label) => {
                    if let Some(f) = menus::filter_of(&label) {
                        s.toggle_filter(f);
                    }
                }
                A::ClearFilters => s.clear_search(),
                _ => {}
            });
            let fresh = menus::filter_entries(ui::store(cx));
            if let Some(menu) = me.borrow().as_ref().and_then(WeakEntity::upgrade) {
                cx.defer(move |cx| menu.update(cx, |m, cx| m.set_entries(fresh, cx)));
            }
        },
        cx,
    );
    *cell.borrow_mut() = menu.map(|m| m.downgrade());
}
