use gpui::{
    AnyElement, App, AppContext, Entity, FontWeight, Hsla, InteractiveElement, IntoElement,
    ParentElement, SharedString, StatefulInteractiveElement, Styled, div, prelude::FluentBuilder,
    px,
};
use mx_core::model::{ClaudeStatus, Tab, TabKind};
use mx_store::Store;

use crate::app::avatar::{claude_avatar, dot};
use crate::app::drag::{DraggedTab, TabGhost};
use crate::app::host::{StoreHost, update_store};
use crate::app::icons::{Icon, icon};
use crate::app::keys;
use crate::app::theme::Mx;
use crate::app::tooltip::tip;

pub const HEIGHT: f32 = 42.;

pub struct Header {
    id: String,
    title: String,
    subtitle: String,
    branch: Option<String>,
    kind: TabKind,
    status: ClaudeStatus,
    running: bool,
    slot: Option<String>,
    dirty: usize,
    relaunch: Option<String>,
    hibernated: bool,
    agent: bool,
    pin: Option<bool>,
    done: bool,
    dim: bool,
    tint: Option<Hsla>,
    folder_root: String,
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
            subtitle: store.subtitle(t),
            branch: (!t.branch.is_empty() && t.branch != title).then(|| t.branch.clone()),
            title,
            kind: t.kind,
            status: t.status(),
            running: !t.exited,
            slot: store.tabs().iter().position(|x| x.id == t.id).and_then(keys::slot),
            dirty: t.dirty,
            relaunch: launcher.filter(|_| t.exited).map(|l| l.command.clone()),
            hibernated: t.hibernated,
            agent: t.kind == TabKind::Claude,
            pin: (t.pinned || store.pane_count() > 1).then_some(t.pinned),
            done: t.done,
            dim: show_focus && !focused,
            tint: store.tint_of(&t.id).map(|tt| mx.tab_tint(tt)),
            folder_root: t.folder_root.clone(),
        }
    }

    fn button(
        &self,
        mx: &Mx,
        (name, glyph, color): (&str, Icon, Hsla),
        tooltip: impl Into<SharedString>,
        host: &Entity<StoreHost>,
        act: impl Fn(&mut Store, &str) + 'static,
    ) -> AnyElement {
        let host = host.clone();
        let id = self.id.clone();
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
            .on_click(move |_, _, cx: &mut App| {
                cx.stop_propagation();
                update_store(&host, cx, |s| act(s, &id))
            })
            .child(icon(glyph, 16., color))
            .into_any_element()
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
            buttons.push(self.button(mx, ("agent", Icon::Fingerprint, faint), "Copiar o nome da sessão", host, |_, _| {}));
        }
        if let Some(pinned) = self.pin {
            let (glyph, color) = if pinned { (Icon::PushPin, mx.accent) } else { (Icon::PushPinOutlined, faint) };
            buttons.push(self.button(mx, ("pin", glyph, color), if pinned { "Solto" } else { "Prender" }, host, |s, id| s.toggle_pin(id)));
        }
        buttons.push(self.button(mx, ("reveal", Icon::MyLocation, faint), "mostrar na lateral", host, |s, id| s.reveal_tab(id)));
        if self.agent {
            let done = self.done;
            let (glyph, color, tooltip) = if done {
                (Icon::CheckCircle, mx.green, "Concluída — clique pra reabrir")
            } else {
                (Icon::CheckCircleOutline, faint, "Marcar esta sessão como concluída")
            };
            buttons.push(self.button(mx, ("done", glyph, color), tooltip, host, move |s, id| {
                s.set_done(id, !done);
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
        if self.dirty > 0 {
            chips = chips.child(div().flex_none().text_size(px(11.)).text_color(mx.yellow).child(format!("±{}", self.dirty)));
        }
        chips = chips.child(div().min_w_0().truncate().text_size(px(12.)).text_color(mx.fg_dim).child(self.subtitle.clone()));
        let fade = if self.dim { 0.55 } else { 1. };
        let drag = DraggedTab {
            id: self.id.clone(),
            title: self.title.clone(),
            folder_root: self.folder_root.clone(),
        };
        div()
            .id(SharedString::from(format!("pane-header:{}", self.id)))
            .h(px(HEIGHT))
            .flex_none()
            .pl(px(12.))
            .pr(px(8.))
            .flex()
            .items_center()
            .bg(mx.bg_sidebar)
            .border_b(px(if self.tint.is_some() { 3. } else { 1. }))
            .border_color(self.tint.unwrap_or(mx.border))
            .on_drag(drag, |d: &DraggedTab, at, _, cx| {
                cx.new(|_| TabGhost {
                    title: d.title.clone(),
                    at,
                })
            })
            .child(lead)
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
            .when_some(self.slot.clone(), |d, slot| {
                d.child(div().flex_none().ml(px(6.)).font_family("Hack").text_size(px(10.5)).text_color(mx.fg_faint).child(slot))
            })
            .child(div().ml(px(10.)).flex_1().min_w_0().flex().opacity(fade).child(chips))
            .child(div().flex_none().flex().items_center().gap(px(2.)).opacity(fade).children(buttons))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_close_tooltip_names_the_chord_that_ends_the_session() {
        assert_eq!(
            close_tooltip(),
            "Tirar do painel — a sessão continua na lateral\nCtrl+Shift+W encerra a sessão"
        );
    }
}
