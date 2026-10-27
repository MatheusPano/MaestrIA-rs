use gpui::{
    AnyElement, App, ClickEvent, ClipboardItem, Entity, FontWeight, Hsla, InteractiveElement, IntoElement,
    Modifiers, ParentElement, SharedString, StatefulInteractiveElement, Styled, Window, div,
    px,
};
use mx_core::model::{ClaudeStatus, Tab, TabKind};
use mx_store::Store;

use crate::app::avatar::{claude_avatar, dot};
use crate::app::drag::{DraggedTab, pane_grab};
use crate::app::host::{StoreHost, update_store};
use crate::app::icons::{Icon, icon};
use crate::app::keys;
use crate::app::theme::Mx;
use crate::app::tooltip::tip;

pub const HEIGHT: f32 = 42.;

pub struct Header {
    id: String,
    title: String,
    branch: Option<String>,
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
            branch: (!t.branch.is_empty() && t.branch != title).then(|| t.branch.clone()),
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
        }
    }

    pub fn lifted(mut self, lifted: bool) -> Self {
        self.lifted = lifted;
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

    pub fn render(self, mx: &Mx, host: &Entity<StoreHost>) -> impl IntoElement {
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
            TabKind::Claude => claude_avatar(mx, self.status, 22., self.done).into_any_element(),
            _ => dot(if self.running { mx.green } else { mx.fg_faint }, 8.).into_any_element(),
        };
        let mut chips = div().flex().items_center().gap(px(6.)).min_w_0().flex_1();
        if let Some(branch) = &self.branch {
            chips = chips.child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(px(3.))
                    .child(icon(Icon::CallSplit, 12., mx.fg_dim))
                    .child(div().font_family("Hack").text_size(px(11.)).text_color(mx.fg_dim).child(branch.clone())),
            );
        }
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
