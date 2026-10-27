use gpui::{App, AppContext, Context, Entity, Global, Pixels, Point, Window};
use mx_term::{LinkFollow, follow_link};

use crate::app::host::{StoreHost, update_store};
use crate::chrome::icon;
use crate::chrome::menu::panel::terminal_menu;
use crate::chrome::menu::{ContextMenu, MenuAction, MenuEntry, MenuItem, Tone};
use crate::keymap::{self, LINUX};
use crate::terminal_view::{TerminalView, TerminalViewEvent};

pub struct TermHost(pub Entity<StoreHost>);

impl Global for TermHost {}

pub fn install(host: &Entity<StoreHost>, cx: &mut App) {
    cx.set_global(TermHost(host.clone()));
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TermMenu {
    Copy,
    Paste,
    SelectAll,
    OpenLink(String),
}

pub fn menu_entries(has_selection: bool, link: Option<String>, linux: bool) -> Vec<MenuEntry<TermMenu>> {
    let copy = keymap::copy_chord(linux).label(linux);
    let paste = keymap::paste_chords(linux)[0].label(linux);
    let mut out: Vec<MenuEntry<TermMenu>> = terminal_menu(&copy, &paste, has_selection)
        .into_iter()
        .filter_map(|e| match e {
            MenuEntry::Item(item) => {
                let value = match item.value.as_ref()? {
                    MenuAction::Copy => TermMenu::Copy,
                    MenuAction::Paste => TermMenu::Paste,
                    _ => return None,
                };
                Some(MenuEntry::Item(retag(item, value)))
            }
            _ => None,
        })
        .collect();
    out.push(
        MenuItem::new("Selecionar tudo", TermMenu::SelectAll)
            .dot(icon::DONE_ALL_ROUNDED, Tone::Dim, 13.)
            .chord(Some(keymap::select_all_chord(linux).label(linux)))
            .into(),
    );
    if let Some(href) = link {
        out.push(MenuEntry::Divider);
        out.push(MenuItem::new("Abrir link", TermMenu::OpenLink(href)).dot(icon::OPEN_IN_NEW, Tone::Dim, 13.).into());
    }
    out
}

fn retag(item: MenuItem<MenuAction>, value: TermMenu) -> MenuItem<TermMenu> {
    MenuItem {
        label: item.label,
        glyph: item.glyph,
        subtitle: item.subtitle,
        chord: item.chord,
        trailing: item.trailing,
        tone: item.tone,
        disabled: item.disabled,
        keep_open: item.keep_open,
        indent: item.indent,
        value: Some(value),
    }
}

pub fn default_menu(has_selection: bool, link: Option<String>) -> Vec<MenuEntry<TermMenu>> {
    menu_entries(has_selection, link, LINUX)
}

pub fn follow(href: &str, tab: Option<&str>, cwd: Option<&str>, cx: &mut App) {
    if let Some(host) = cx.try_global::<TermHost>().map(|h| h.0.clone()) {
        update_store(&host, cx, |s| s.follow_link(href, tab, cwd));
        return;
    }
    match follow_link(href, cwd) {
        LinkFollow::OpenUrl(target) | LinkFollow::OpenFile(target) | LinkFollow::Reader(target) => open_external(&target),
        LinkFollow::Missing(_) | LinkFollow::Nothing => {}
    }
}

fn open_external(target: &str) {
    let opener = if LINUX { "xdg-open" } else { "open" };
    match std::process::Command::new(opener).arg(target).spawn() {
        Ok(mut child) => {
            std::thread::spawn(move || child.wait());
        }
        Err(err) => log::warn!("{opener} {target}: {err}"),
    }
}

impl TerminalView {
    pub fn follow(&mut self, href: &str, cx: &mut Context<Self>) {
        cx.emit(TerminalViewEvent::Link(href.to_owned()));
        let tab = self.session.tab_id();
        follow(href, tab.as_deref(), self.cwd.as_deref(), cx);
    }

    pub fn open_menu(&mut self, at: Point<Pixels>, link: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        let entries = default_menu(self.session.has_selection(), link);
        let picked = cx.entity().downgrade();
        let dismissed = picked.clone();
        let menu = cx.new(|_| {
            ContextMenu::new(
                entries,
                at,
                move |action, window, cx| {
                    picked.update(cx, |v, cx| v.menu_pick(action, window, cx)).ok();
                },
                move |window, cx| {
                    dismissed.update(cx, |v, cx| v.close_menu(window, cx)).ok();
                },
            )
        });
        self.menu = Some(menu);
        window.focus(&self.focus);
        cx.notify();
    }

    pub fn close_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.menu.take().is_some() {
            window.focus(&self.focus);
            cx.notify();
        }
    }

    fn menu_pick(&mut self, action: TermMenu, _window: &mut Window, cx: &mut Context<Self>) {
        match action {
            TermMenu::Copy => self.copy_selection(cx),
            TermMenu::Paste => self.paste_clipboard(cx),
            TermMenu::SelectAll => self.select_all(cx),
            TermMenu::OpenLink(href) => self.follow(&href, cx),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(e: &[MenuEntry<TermMenu>]) -> Vec<String> {
        e.iter().filter_map(|x| x.label().map(str::to_owned)).collect()
    }

    #[test]
    fn right_click_menu_offers_copy_paste_and_select_all() {
        let e = menu_entries(true, None, true);
        assert_eq!(labels(&e), ["Copiar", "Colar", "Selecionar tudo"]);
        let MenuEntry::Item(copy) = &e[0] else { panic!() };
        assert_eq!(copy.chord.as_ref().map(|c| c.as_ref()), Some("Ctrl+Shift+C"));
        assert_eq!(copy.value, Some(TermMenu::Copy));
        let MenuEntry::Item(paste) = &e[1] else { panic!() };
        assert_eq!(paste.chord.as_ref().map(|c| c.as_ref()), Some("Ctrl+Shift+V"));
    }

    #[test]
    fn copy_is_disabled_without_a_selection() {
        let e = menu_entries(false, None, false);
        let MenuEntry::Item(copy) = &e[0] else { panic!() };
        assert!(copy.disabled);
        assert_eq!(copy.chord.as_ref().map(|c| c.as_ref()), Some("⌘C"));
    }

    #[test]
    fn a_link_under_the_pointer_adds_open_link() {
        let e = menu_entries(false, Some("https://x.dev".into()), true);
        assert_eq!(labels(&e).last().map(String::as_str), Some("Abrir link"));
        let Some(MenuEntry::Item(open)) = e.last() else { panic!() };
        assert_eq!(open.value, Some(TermMenu::OpenLink("https://x.dev".into())));
    }
}
