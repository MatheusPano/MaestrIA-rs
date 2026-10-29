use gpui::{
    AnyElement, App, FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, div, px,
};
use mx_store::Store;
use mx_sys::updater::{INSTALLS_ON_QUIT, Stage};

use crate::app::theme::Mx;
use crate::app::tooltip::tip;
use crate::chrome::icon::{self, icon};

pub fn label(version: &str, installing: bool) -> String {
    if installing { "instalando…".into() } else { version.to_owned() }
}

pub fn tooltip(version: &str, installing: bool, installs_on_quit: bool) -> String {
    if installing {
        format!("Instalando a {version}…")
    } else if installs_on_quit {
        format!("maestria {version} pronta · entra quando você fechar o app")
    } else {
        format!("maestria {version} pronta pra instalar")
    }
}

pub fn chip(store: &Store, mx: &Mx) -> Option<AnyElement> {
    let u = store.app_updater();
    if !u.shows_chip() {
        return None;
    }
    let version = u.release()?.version.clone();
    let installing = u.stage() == Stage::Installing;
    let (fg, accent, bg_hover) = (mx.fg, mx.accent, mx.bg_hover);
    let mut el = div()
        .id("status-update")
        .debug_selector(|| "status-update".into())
        .group("status-update")
        .h(px(20.))
        .mr(px(4.))
        .px(px(6.))
        .rounded(px(4.))
        .flex()
        .items_center()
        .gap(px(4.))
        .hover(move |s| s.bg(bg_hover))
        .tooltip(tip(SharedString::from(tooltip(&version, installing, INSTALLS_ON_QUIT))))
        .child(icon(icon::ARROW_CIRCLE_UP_ROUNDED, 14., accent).group_hover("status-update", move |s| s.text_color(fg)))
        .child(
            div()
                .text_size(px(11.))
                .line_height(px(11.))
                .font_weight(FontWeight::BOLD)
                .text_color(accent)
                .group_hover("status-update", move |s| s.text_color(fg))
                .child(label(&version, installing)),
        );
    if !installing {
        el = el.cursor_pointer().on_click(|_, _, cx: &mut App| crate::app::update_confirm::confirm(cx));
    }
    Some(el.into_any_element())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_chip_shows_the_version_or_that_it_is_installing() {
        assert_eq!(label("3.1.0", false), "3.1.0");
        assert_eq!(label("3.1.0", true), "instalando…");
    }

    #[test]
    fn the_tooltip_says_when_the_version_lands() {
        assert_eq!(tooltip("3.1.0", true, false), "Instalando a 3.1.0…");
        assert_eq!(tooltip("3.1.0", false, true), "maestria 3.1.0 pronta · entra quando você fechar o app");
        assert_eq!(tooltip("3.1.0", false, false), "maestria 3.1.0 pronta pra instalar");
    }
}
