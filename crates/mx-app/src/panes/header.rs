use gpui::{
    AnyElement, App, ClickEvent, ClipboardItem, Entity, FontWeight, Hsla, InteractiveElement, IntoElement,
    Modifiers, ParentElement, prelude::FluentBuilder, SharedString, StatefulInteractiveElement, Styled, WeakEntity,
    Window, div, px,
};
use mx_claude::{ChipLevel, ContextUsage};
use mx_core::model::{ClaudeStatus, Tab, TabKind};
use mx_store::Store;

use crate::app::avatar::{claude_avatar, dot};
use crate::app::drag::{DraggedTab, pane_grab};
use crate::app::host::{StoreHost, update_store};
use crate::app::icons::{Icon, icon};
use crate::app::keys;
use crate::app::theme::{Mx, RADIUS};
use crate::app::tooltip::tip;
use crate::chrome::icon as chrome_icon;
use crate::panes::PaneArea;

pub const HEIGHT: f32 = 42.;

#[derive(Clone, PartialEq)]
pub struct Header {
    id: String,
    title: String,
    feature: Option<String>,
    branch: Option<String>,
    context: Option<ContextUsage>,
    kind: TabKind,
    status: ClaudeStatus,
    running: bool,
    hotkey: Option<String>,
    relaunch: Option<String>,
    hibernated: bool,
    agent: bool,
    name: Option<String>,
    session: Option<String>,
    twins: usize,
    pin: Option<bool>,
    done: bool,
    dim: bool,
    tint: Option<Hsla>,
    folder_root: String,
    grab: bool,
    lifted: bool,
    files: usize,
    files_open: bool,
}

pub fn close_tooltip() -> String {
    format!(
        "Tirar do painel — a sessão continua na lateral\n{} encerra a sessão",
        keys::CLOSE_PANE
    )
}

impl Header {
    pub fn of(store: &Store, mx: &Mx, t: &Tab, show_focus: bool, focused: bool) -> Header {
        let title = store.title(t);
        let launcher = t.launcher_id.as_deref().and_then(|l| store.launcher_by_id(l));
        Header {
            id: t.id.clone(),
            feature: store.feature_of(t).map(|f| f.name.clone()),
            branch: (!t.branch.is_empty() && t.branch != title).then(|| t.branch.clone()),
            context: t.context.filter(|_| t.kind == TabKind::Claude).map(|c| ContextUsage {
                percent: c.percent,
                tokens: c.tokens,
                window: c.window,
            }),
            title,
            kind: t.kind,
            status: t.status(),
            running: !t.exited,
            hotkey: t.hotkey.as_deref().map(mx_store::hotkey_label),
            relaunch: launcher.filter(|_| t.exited).map(|l| l.command.clone()),
            hibernated: t.hibernated,
            agent: t.kind == TabKind::Claude,
            name: t.agent_name.clone(),
            session: t.resume_id().map(str::to_owned),
            twins: store
                .tabs()
                .iter()
                .filter(|x| x.id != t.id && !x.exited && x.agent_name.is_some() && x.agent_name == t.agent_name)
                .count(),
            pin: (t.pinned || store.pane_count() > 1).then_some(t.pinned),
            done: t.done,
            dim: show_focus && !focused,
            tint: store.tint_of(&t.id).map(|tt| mx.tab_tint(tt)),
            folder_root: t.folder_root.clone(),
            grab: store.pane_count() >= 2,
            lifted: false,
            files: t.hooks.touched.len(),
            files_open: false,
        }
    }

    pub fn lifted(mut self, lifted: bool) -> Self {
        self.lifted = lifted;
        self
    }

    pub fn files_open(mut self, open: bool) -> Self {
        self.files_open = open;
        self
    }

    fn button(
        &self,
        mx: &Mx,
        look: (&str, Icon, Hsla),
        tooltip: impl Into<SharedString>,
        host: &Entity<StoreHost>,
        act: impl Fn(&mut Store, &str) + 'static,
    ) -> AnyElement {
        let host = host.clone();
        let id = self.id.clone();
        self.button_with(mx, look, tooltip, move |_, _, cx| update_store(&host, cx, |s| act(s, &id)))
    }

    fn button_with(
        &self,
        mx: &Mx,
        (name, glyph, color): (&str, Icon, Hsla),
        tooltip: impl Into<SharedString>,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> AnyElement {
        div()
            .id(SharedString::from(format!("pane-{name}:{}", self.id)))
            .flex_none()
            .size(px(26.))
            .rounded(px(6.))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .hover(|s| s.bg(mx.bg_hover))
            .tooltip(tip(tooltip.into()))
            .on_click(move |ev, window, cx: &mut App| {
                cx.stop_propagation();
                on_click(ev, window, cx)
            })
            .child(icon(glyph, 16., color))
            .into_any_element()
    }

    fn agent_button(&self, mx: &Mx, host: &Entity<StoreHost>) -> AnyElement {
        let tooltip = agent_tooltip(self.name.as_deref(), self.session.as_deref());
        let (host, name, session, twins) = (host.clone(), self.name.clone(), self.session.clone(), self.twins);
        self.button_with(mx, ("agent", Icon::Fingerprint, mx.fg_faint), tooltip, move |ev, _, cx| {
            let Some((value, banner)) = agent_copy(name.as_deref(), session.as_deref(), twins, ev.modifiers()) else {
                return;
            };
            cx.write_to_clipboard(ClipboardItem::new_string(value));
            update_store(&host, cx, |s| s.show_banner(&banner));
        })
    }

    pub fn render(self, mx: &Mx, host: &Entity<StoreHost>, area: &WeakEntity<PaneArea>) -> impl IntoElement {
        let faint = mx.fg_faint;
        let mut buttons: Vec<AnyElement> = Vec::new();
        if let Some(cmd) = &self.relaunch {
            buttons.push(self.button(mx, ("relaunch", Icon::Replay, faint), format!("Rodar {cmd} de novo"), host, |s, id| s.relaunch(id)));
        }
        if self.hibernated {
            buttons.push(self.button(mx, ("wake", Icon::PlayArrow, faint), "Retomar a conversa", host, |s, id| s.wake(id)));
        }
        if self.agent {
            buttons.push(self.agent_button(mx, host));
        }
        if let Some(pinned) = self.pin {
            let (glyph, color) = if pinned { (Icon::PushPin, mx.accent) } else { (Icon::PushPinOutlined, faint) };
            buttons.push(self.button(mx, ("pin", glyph, color), if pinned { "Solto" } else { "Prender" }, host, |s, id| s.toggle_pin(id)));
        }
        buttons.push(self.button(mx, ("reveal", Icon::MyLocation, faint), "mostrar na lateral", host, |s, id| s.reveal_tab(id)));
        buttons.push(more_button(mx, &self.id));
        if self.agent {
            let done = self.done;
            let (glyph, color, tooltip) = if done {
                (Icon::CheckCircle, mx.green, "Concluída — clique pra reabrir")
            } else {
                (Icon::CheckCircleOutline, faint, "Marcar esta sessão como concluída")
            };
            let id = self.id.clone();
            buttons.push(self.button_with(mx, ("done", glyph, color), tooltip, move |_, window, cx| {
                crate::app::panel_actions::set_done(&id, !done, window, cx)
            }));
        }
        buttons.push(self.button(mx, ("close", Icon::CloseRounded, faint), close_tooltip(), host, |s, id| s.dismiss(id)));

        let lead: AnyElement = match self.kind {
            TabKind::Claude => claude_avatar(mx, self.status, 22., self.done, self.hibernated).into_any_element(),
            _ => dot(if self.running { mx.green } else { mx.fg_faint }, 8.).into_any_element(),
        };
        let mut chips: Vec<AnyElement> = Vec::new();
        if let Some(feature) = &self.feature {
            chips.push(text_chip(mx.purple, Icon::TrackChanges, div().text_size(px(11.)).child(feature.clone())));
        }
        if let Some(branch) = &self.branch {
            let muted = mx.fg_muted;
            chips.push(text_chip(muted, Icon::CallSplit, div().font_family("Hack").text_size(px(11.)).child(branch.clone())));
        }
        if let Some(usage) = &self.context {
            chips.push(context_chip(mx, &self.id, usage));
        }
        if self.files > 0 {
            chips.push(files_chip(mx, &self.id, self.files, self.files_open, area));
        }
        let chips = div().flex().items_center().min_w_0().flex_1().children(dotted(mx, chips));
        let fade = if self.dim { 0.55 } else { 1. };
        let drag = DraggedTab {
            id: self.id.clone(),
            title: self.title.clone(),
            folder_root: self.folder_root.clone(),
        };
        let el = div()
            .id(SharedString::from(format!("pane-header:{}", self.id)))
            .h(px(HEIGHT))
            .flex_none()
            .pl(px(12.))
            .pr(px(8.))
            .flex()
            .items_center()
            .rounded_t(px(RADIUS))
            .bg(mx.bg_sidebar)
            .border_b(px(if self.tint.is_some() { 3. } else { 1. }))
            .border_color(self.tint.unwrap_or(mx.border));
        let el = if self.grab { pane_grab(el, drag, self.tint, host.clone(), self.lifted) } else { el };
        el.child(lead)
            .child(div().w(px(9.)))
            .child(
                div()
                    .flex_none()
                    .max_w(px(320.))
                    .truncate()
                    .text_size(px(15.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(mx.fg)
                    .child(self.title.clone()),
            )
            .children(key_hint(mx, self.hotkey.clone()))
            .child(div().ml(px(10.)).flex_1().min_w_0().flex().opacity(fade).child(chips))
            .child(div().flex_none().flex().items_center().gap(px(2.)).opacity(fade).children(buttons))
    }
}

fn dotted(mx: &Mx, items: Vec<AnyElement>) -> Vec<AnyElement> {
    let mut out = Vec::new();
    for (i, item) in items.into_iter().enumerate() {
        if i > 0 {
            out.push(div().flex_none().px(px(7.)).text_size(px(11.)).text_color(mx.fg_faint).child("·").into_any_element());
        }
        out.push(item);
    }
    out
}

fn text_chip(color: Hsla, glyph: Icon, text: gpui::Div) -> AnyElement {
    div()
        .flex_none()
        .flex()
        .items_center()
        .gap(px(4.))
        .child(icon(glyph, 11., color))
        .child(text.text_color(color))
        .into_any_element()
}

pub fn files_label(n: usize) -> String {
    if n == 1 { "1 arquivo".to_owned() } else { format!("{n} arquivos") }
}

fn files_chip(mx: &Mx, tab: &str, count: usize, open: bool, area: &WeakEntity<PaneArea>) -> AnyElement {
    let color = if open { mx.accent } else { mx.fg_dim };
    let fg = mx.fg;
    let tooltip = if open { "Esconder as mudanças desta sessão" } else { "Ver as mudanças desta sessão, arquivo por arquivo" };
    let (area, id) = (area.clone(), tab.to_owned());
    div()
        .id(SharedString::from(format!("pane-files:{tab}")))
        .group("pane-files")
        .flex_none()
        .flex()
        .items_center()
        .pl(px(6.))
        .pr(px(6.))
        .py(px(2.))
        .rounded(px(5.))
        .cursor_pointer()
        .when(open, |d| d.bg(mx.accent.opacity(0.12)))
        .when(!open, |d| d.hover(|s| s.bg(mx.bg_hover)))
        .tooltip(tip(tooltip))
        .on_click(move |_, _, cx: &mut App| {
            cx.stop_propagation();
            area.update(cx, |a, cx| a.toggle_changes(&id, cx)).ok();
        })
        .child(
            chrome_icon::icon(chrome_icon::EDIT_OUTLINED, 14., color)
                .when(!open, |d| d.group_hover("pane-files", move |s| s.text_color(fg))),
        )
        .child(div().w(px(4.)))
        .child(
            div()
                .text_size(px(11.))
                .text_color(color)
                .when(!open, |d| d.group_hover("pane-files", move |s| s.text_color(fg)))
                .child(files_label(count)),
        )
        .into_any_element()
}

fn context_chip(mx: &Mx, tab: &str, usage: &ContextUsage) -> AnyElement {
    let color = match usage.chip_level() {
        ChipLevel::Danger => mx.red,
        ChipLevel::Warn => mx.yellow,
        ChipLevel::Calm => mx.fg_dim,
    };
    let filled = usage.chip_filled();
    div()
        .id(SharedString::from(format!("pane-context:{tab}")))
        .flex_none()
        .flex()
        .items_center()
        .py(px(3.))
        .tooltip(tip(usage.chip_tooltip()))
        .children((0..5).map(|i| {
            div()
                .w(px(4.))
                .h(px(7.))
                .when(i < 4, |d| d.mr(px(1.)))
                .bg(color.opacity(if i < filled { 1. } else { 0.25 }))
        }))
        .child(div().w(px(4.)))
        .child(div().text_size(px(11.)).text_color(color).child(usage.chip_text()))
        .into_any_element()
}

pub fn key_hint(mx: &Mx, label: Option<String>) -> Option<gpui::Div> {
    let label = label?;
    Some(div().flex_none().pl(px(8.)).pr(px(2.)).font_family("Hack").text_size(px(10.5)).text_color(mx.fg_faint).child(label))
}

fn held_for_id(m: Modifiers) -> bool {
    if cfg!(target_os = "linux") { m.control } else { m.alt }
}

pub fn agent_tooltip(name: Option<&str>, session: Option<&str>) -> String {
    let click = if cfg!(target_os = "linux") { "Ctrl+clique" } else { "⌥ clique" };
    let mut lines = Vec::new();
    if let Some(name) = name {
        lines.push(format!("Nome: {name}  ·  clique pra copiar"));
    }
    if let Some(id) = session {
        lines.push(format!("Sessão: {id}  ·  {click} pra copiar"));
    }
    lines.join("\n")
}

pub fn agent_copy(name: Option<&str>, session: Option<&str>, twins: usize, m: Modifiers) -> Option<(String, String)> {
    let wants_id = held_for_id(m) || name.is_none();
    if wants_id {
        let id = session?;
        return Some((id.to_owned(), format!("Id da sessão copiado: {id}")));
    }
    let name = name?;
    let banner = if twins == 0 {
        format!("Nome copiado: {name} — outro agente fala com esta sessão por esse nome")
    } else {
        format!("Nome copiado: {name} — cuidado: mais {twins} sessão(ões) atendem por esse mesmo nome")
    };
    Some((name.to_owned(), banner))
}

fn more_button(mx: &Mx, tab: &str) -> AnyElement {
    let tab = tab.to_owned();
    div()
        .id(SharedString::from(format!("pane-more:{tab}")))
        .flex_none()
        .size(px(26.))
        .rounded(px(6.))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .hover(|s| s.bg(mx.bg_hover))
        .tooltip(tip("Mais"))
        .on_click(move |ev, _, cx: &mut App| {
            cx.stop_propagation();
            crate::app::panel_actions::open_panel_menu(&tab, ev.position(), cx);
        })
        .child(icon(Icon::MoreHoriz, 16., mx.fg_faint))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clicking_the_fingerprint_copies_the_name_and_says_who_else_answers_to_it() {
        let plain = Modifiers::none();
        assert_eq!(
            agent_copy(Some("api"), Some("abc"), 0, plain),
            Some(("api".into(), "Nome copiado: api — outro agente fala com esta sessão por esse nome".into()))
        );
        assert_eq!(
            agent_copy(Some("api"), Some("abc"), 2, plain).unwrap().1,
            "Nome copiado: api — cuidado: mais 2 sessão(ões) atendem por esse mesmo nome"
        );
    }

    #[test]
    fn ctrl_click_or_a_nameless_session_copies_the_id() {
        let ctrl = Modifiers { control: true, ..Modifiers::none() };
        assert_eq!(agent_copy(Some("api"), Some("abc"), 0, ctrl), Some(("abc".into(), "Id da sessão copiado: abc".into())));
        assert_eq!(agent_copy(None, Some("abc"), 0, Modifiers::none()).unwrap().0, "abc");
        assert_eq!(agent_copy(None, None, 0, Modifiers::none()), None);
    }

    #[test]
    fn the_fingerprint_tooltip_has_one_line_per_thing_it_can_copy() {
        assert_eq!(
            agent_tooltip(Some("api"), Some("abc")),
            "Nome: api  ·  clique pra copiar\nSessão: abc  ·  Ctrl+clique pra copiar"
        );
        assert_eq!(agent_tooltip(Some("api"), None), "Nome: api  ·  clique pra copiar");
    }

    #[test]
    fn the_close_tooltip_names_the_chord_that_ends_the_session() {
        assert_eq!(
            close_tooltip(),
            "Tirar do painel — a sessão continua na lateral\nCtrl+Shift+W encerra a sessão"
        );
    }
}
