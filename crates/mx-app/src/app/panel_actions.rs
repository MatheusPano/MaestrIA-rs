use gpui::{App, Pixels, Point, Window};

use crate::app::menus::{Target, core_tint};
use crate::app::{dialogs, flows, menus, ui};
use crate::chrome::menu::MenuAction as A;

pub fn open_panel_menu(tab: &str, at: Point<Pixels>, cx: &mut App) {
    menus::open_at(Target::Panel(tab.to_owned()), at, cx);
}

pub fn done_banner(title: &str, dropped: usize) -> String {
    format!("{title}: concluída — {dropped} passo(s) da fila foram descartados")
}

pub fn set_done(id: &str, done: bool, window: &mut Window, cx: &mut App) {
    let at = window.mouse_position();
    ui::act(cx, |s| {
        let dropped = s.set_done(id, done);
        if dropped > 0 {
            let title = s.title_of(id);
            s.show_banner(&done_banner(&title, dropped));
        }
    });
    if done {
        ui::puff(at, window, cx);
    }
}

pub fn run(id: &str, action: A, window: &mut Window, cx: &mut App) {
    let id = id.to_owned();
    match action {
        A::Relaunch => ui::act(cx, |s| s.relaunch(&id)),
        A::Wake => ui::act(cx, |s| s.wake(&id)),
        A::Hibernate => ui::act(cx, |s| s.hibernate(&id)),
        A::Rename => {
            let title = ui::store(cx).title_of(&id);
            dialogs::prompt("Renomear painel", "Título", &title, move |text, cx| ui::act(cx, |s| s.rename_tab(&id, &text)), window, cx);
        }
        A::Hotkey => crate::chrome::dialog::hotkey::open(&id, window, cx),
        A::Tint(t) => ui::act(cx, |s| s.set_tab_tint(&id, core_tint(t))),
        A::ShowPlan => drop(ui::act(cx, |s| s.show_plan(&id, None))),
        A::ShowMessage => drop(ui::act(cx, |s| s.show_message(&id))),
        A::Flow => flows::existing_flow(&id, window, cx),
        A::TogglePin => ui::act(cx, |s| s.toggle_pin(&id)),
        A::GroupPanes => ui::act(cx, |s| {
            if let Some(g) = s.group_panes().and_then(|g| s.group_by_id(&g).cloned()) {
                s.show_banner(&format!("{} painéis agrupados como \"{}\"", g.count(), g.name));
            }
        }),
        A::Ungroup => ui::act(cx, |s| {
            if let Some(g) = s.group_of(&id).map(|g| g.id.clone()) {
                s.ungroup(&g);
            }
        }),
        A::MoveTo => dialogs::move_to(&id, cx),
        A::SetDone(done) => set_done(&id, done, window, cx),
        A::PluginCommand(command) => ui::act(cx, |s| s.run_plugin_command(&command, Some(&id))),
        A::Close => drop(ui::act(cx, |s| s.close_tab(&id))),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marking_done_names_how_many_queued_steps_were_dropped() {
        assert_eq!(done_banner("Lint", 3), "Lint: concluída — 3 passo(s) da fila foram descartados");
    }
}
