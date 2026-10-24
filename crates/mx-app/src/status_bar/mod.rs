mod sessions;

use gpui::{
    App, Context, Entity, FontWeight, InteractiveElement, IntoElement, ParentElement, Render,
    SharedString, StatefulInteractiveElement, Styled, Window, div, px,
};
use mx_core::model::{ClaudeStatus, TabKind};
use mx_store::Store;

use crate::app::host::{StoreHost, update_store};
use crate::app::icons::{Icon, icon};
use crate::app::theme::{GAP, Mx};
use crate::app::tooltip::tip;

pub use sessions::SessionsView;

pub const HEIGHT: f32 = 26.;
pub const INSET: f32 = GAP + 36. + GAP;

pub struct StatusBar {
    host: Entity<StoreHost>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Summary {
    pub sessions: usize,
    pub working: usize,
    pub waiting: usize,
}

impl Summary {
    pub fn of(store: &Store) -> Summary {
        let live = store
            .tabs()
            .iter()
            .filter(|t| t.kind == TabKind::Claude && !t.exited);
        let mut s = Summary::default();
        for t in live {
            s.sessions += 1;
            let status = t.status();
            if matches!(status, ClaudeStatus::Working | ClaudeStatus::Tool) {
                s.working += 1;
            }
            if !t.done && status.needs_human() {
                s.waiting += 1;
            }
        }
        s
    }

    pub fn head(&self) -> String {
        let noun = if self.sessions == 1 { "sessão" } else { "sessões" };
        let mut out = format!("{} {noun}", self.sessions);
        if self.working > 0 {
            out.push_str(&format!("  ·  {} trabalhando", self.working));
        }
        out
    }

    pub fn tail(&self) -> Option<String> {
        (self.waiting > 0).then(|| format!("  ·  {} esperando você", self.waiting))
    }
}

pub fn bell_tooltip(unread: usize, dnd: bool) -> String {
    let mut parts = vec!["Notificações".to_owned()];
    if unread > 0 {
        parts.push(format!("{unread} {}", if unread == 1 { "nova" } else { "novas" }));
    }
    if dnd {
        parts.push("não perturbe".into());
    }
    parts.join(" · ")
}

pub fn bell_count(unread: usize) -> String {
    if unread > 99 { "99+".into() } else { unread.to_string() }
}

impl StatusBar {
    pub fn new(host: Entity<StoreHost>) -> StatusBar {
        StatusBar { host }
    }
}

impl Render for StatusBar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mx = Mx::get(cx).clone();
        let store = &self.host.read(cx).store;
        let summary = Summary::of(store);
        let bell = store.bell();
        let (sessions_open, notices_open) = (bell.sessions_open, bell.notices_open);
        let unread = store.unread_notices();
        let dnd = store.prefs().do_not_disturb;
        let top = store.prefs().status_bar_top;

        let pill = |id: &'static str, open: bool| {
            div()
                .id(id)
                .h(px(20.))
                .px(px(6.))
                .rounded(px(4.))
                .flex()
                .items_center()
                .cursor_pointer()
                .when_open(open, mx.bg_hover)
                .hover(|s| s.bg(mx.bg_hover))
        };

        let text_color = if sessions_open { mx.fg_dim } else { mx.fg_faint };
        let mut summary_text = div()
            .flex()
            .text_size(px(11.5))
            .line_height(px(11.5))
            .text_color(text_color)
            .child(summary.head());
        if let Some(tail) = summary.tail() {
            summary_text = summary_text.child(div().text_color(mx.yellow).child(tail));
        }
        let host = self.host.clone();
        let sessions = pill("status-sessions", sessions_open)
            .child(summary_text)
            .on_click(move |_, _, cx: &mut App| update_store(&host, cx, Store::toggle_sessions));

        let glyph = if dnd {
            Icon::NotificationsOff
        } else if unread == 0 {
            Icon::NotificationsNone
        } else {
            Icon::Notifications
        };
        let lit = notices_open || unread > 0;
        let color = if lit { mx.accent } else { mx.fg_dim };
        let mut bell_el = pill("status-bell", notices_open)
            .gap(px(3.))
            .child(icon(glyph, 15., color))
            .tooltip(tip(SharedString::from(bell_tooltip(unread, dnd))));
        if unread > 0 {
            bell_el = bell_el.child(
                div()
                    .text_size(px(11.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(color)
                    .child(bell_count(unread)),
            );
        }
        let host = self.host.clone();
        let bell_el =
            bell_el.on_click(move |_, _, cx: &mut App| update_store(&host, cx, Store::toggle_notices));

        let (pt, pb) = if top { (3., 0.) } else { (0., 3.) };
        div()
            .h(px(HEIGHT))
            .w_full()
            .flex_none()
            .flex()
            .items_center()
            .pl(px(INSET))
            .pr(px(12.))
            .pt(px(pt))
            .pb(px(pb))
            .child(sessions)
            .child(div().flex_1())
            .child(bell_el)
    }
}

trait WhenOpen {
    fn when_open(self, open: bool, bg: gpui::Hsla) -> Self;
}

impl<E: Styled> WhenOpen for E {
    fn when_open(self, open: bool, bg: gpui::Hsla) -> Self {
        if open { self.bg(bg) } else { self }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_reads_one_session_in_the_singular() {
        let s = Summary {
            sessions: 1,
            working: 0,
            waiting: 0,
        };
        assert_eq!(s.head(), "1 sessão");
        assert_eq!(s.tail(), None);
    }

    #[test]
    fn summary_lists_working_and_waiting_counts() {
        let s = Summary {
            sessions: 3,
            working: 2,
            waiting: 1,
        };
        assert_eq!(s.head(), "3 sessões  ·  2 trabalhando");
        assert_eq!(s.tail().as_deref(), Some("  ·  1 esperando você"));
    }

    #[test]
    fn bell_tooltip_joins_new_count_and_do_not_disturb() {
        assert_eq!(bell_tooltip(0, false), "Notificações");
        assert_eq!(bell_tooltip(1, false), "Notificações · 1 nova");
        assert_eq!(bell_tooltip(2, true), "Notificações · 2 novas · não perturbe");
    }

    #[test]
    fn bell_count_caps_at_99_plus() {
        assert_eq!(bell_count(99), "99");
        assert_eq!(bell_count(100), "99+");
    }
}
