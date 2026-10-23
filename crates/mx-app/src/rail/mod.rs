use gpui::{
    App, Context, Entity, FontWeight, Hsla, InteractiveElement, IntoElement, ParentElement,
    Render, SharedString, StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder,
    px,
};

use crate::app::host::{StoreHost, update_store};
use crate::app::icons::{Icon, icon};
use crate::app::theme::{GAP, Mx, on_color};
use crate::app::keys;
use crate::app::tooltip::tip;

pub const WIDTH: f32 = 36.;

pub struct Rail {
    host: Entity<StoreHost>,
}

pub fn sessions_tooltip(current: bool, waiting: usize, keys: &[String]) -> String {
    if current {
        let mut parts = vec!["Esconder a lateral".to_owned()];
        parts.extend(keys.iter().cloned());
        return parts.join("  ");
    }
    if waiting > 0 {
        format!("Sessões — {waiting} esperando por você")
    } else {
        "Sessões".into()
    }
}

impl Rail {
    pub fn new(host: Entity<StoreHost>) -> Rail {
        Rail { host }
    }
}

fn rail_button(mx: &Mx, id: &'static str, selected: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .relative()
        .size(px(WIDTH))
        .flex_none()
        .rounded(px(8.))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .border_1()
        .border_color(gpui::transparent_black())
        .when(selected, |d| d.bg(mx.bg_sidebar).border_color(mx.border))
        .when(!selected, |d| d.hover(|s| s.bg(mx.bg_hover)))
}

pub fn text_badge(mx: &Mx, text: String, bg: Hsla) -> impl IntoElement {
    div()
        .h(px(14.))
        .min_w(px(14.))
        .max_w(px(32.))
        .px(px(3.5))
        .rounded(px(7.))
        .bg(bg)
        .border(px(1.5))
        .border_color(mx.canvas)
        .flex()
        .items_center()
        .justify_center()
        .overflow_hidden()
        .text_size(px(8.5))
        .line_height(px(8.5))
        .font_weight(FontWeight::BOLD)
        .text_color(on_color(bg, mx.canvas))
        .whitespace_nowrap()
        .child(text)
}

impl Render for Rail {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mx = Mx::get(cx).clone();
        let store = &self.host.read(cx).store;
        let prefs = store.prefs();
        let current = prefs.sidebar_view.is_none() && !prefs.sidebar_hidden;
        let waiting = store.waiting_sessions();
        let badge = waiting.first().map(|t| mx.status(t.status()));
        let count = waiting.len();
        let tooltip = sessions_tooltip(current, count, &[keys::SIDEBAR.to_owned()]);

        let host = self.host.clone();
        let mut sessions = rail_button(&mx, "rail-sessions", current)
            .child(icon(
                Icon::SpaceDashboard,
                18.,
                if current { mx.fg } else { mx.fg_faint },
            ))
            .tooltip(tip(SharedString::from(tooltip)))
            .on_click(move |_, _, cx: &mut App| {
                update_store(&host, cx, |s| s.show_sidebar_view(None))
            });
        if let (false, Some(color)) = (current, badge) {
            sessions = sessions.child(
                div()
                    .absolute()
                    .right(px(1.))
                    .bottom(px(1.))
                    .child(text_badge(&mx, count.to_string(), color)),
            );
        }

        let install = rail_button(&mx, "rail-install", false)
            .child(icon(Icon::Add, 16., mx.fg_faint.opacity(0.55)))
            .tooltip(tip("Instalar plugin"));

        div()
            .w(px(WIDTH))
            .mr(px(GAP))
            .flex_none()
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(sessions)
            .child(install)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sessions_tooltip_counts_who_is_waiting() {
        assert_eq!(sessions_tooltip(false, 0, &[]), "Sessões");
        assert_eq!(
            sessions_tooltip(false, 2, &[]),
            "Sessões — 2 esperando por você"
        );
    }

    #[test]
    fn sessions_tooltip_offers_to_hide_when_it_is_the_current_view() {
        let keys = vec!["Ctrl+B".to_owned(), "Ctrl+\\".to_owned()];
        assert_eq!(
            sessions_tooltip(true, 3, &keys),
            "Esconder a lateral  Ctrl+B  Ctrl+\\"
        );
    }
}
