use gpui::{App, Entity, FontWeight, IntoElement, ParentElement, Styled, div, px};

use crate::app::drag::DraggedTab;
use crate::app::host::{StoreHost, update_store};
use crate::app::theme::Mx;
use gpui::{InteractiveElement, StatefulInteractiveElement};

pub fn headline(live: usize) -> String {
    match live {
        0 => "Nenhum painel aberto".into(),
        1 => "1 sessão rodando na lateral — clique nela pra trazer de volta".into(),
        n => format!("{n} sessões rodando na lateral — clique numa pra trazer de volta"),
    }
}

pub const DRAG_HINT: &str = "Arraste uma sessão da lateral pra cá — pela borda de um painel, ela divide a tela; pelo meio, toma o lugar dele";

const CHEATS: [(&str, &str); 8] = [
    ("Ctrl+Shift+T", "Nova sessão Claude"),
    ("Ctrl+Shift+Enter", "Novo shell"),
    ("Ctrl+Shift+N", "Nova task"),
    ("Ctrl+Tab  Ctrl+PgDn", "Próximo painel"),
    ("Ctrl+Shift+→", "Próxima sessão"),
    ("Ctrl+Shift+W", "Fechar o painel em foco"),
    ("Ctrl+,  Ctrl+Shift+P", "Configurações"),
    ("Alt+1…9", "Ir pra enésima sessão"),
];

pub fn empty_pane(mx: &Mx, host: &Entity<StoreHost>, live: usize) -> impl IntoElement {
    let host = host.clone();
    let accent = mx.accent;
    let mut sheet = div().mt(px(28.)).flex().flex_col().gap(px(8.));
    for (keys, label) in CHEATS {
        sheet = sheet.child(
            div()
                .flex()
                .gap(px(14.))
                .child(
                    div()
                        .w(px(170.))
                        .flex()
                        .justify_end()
                        .font_family("Hack")
                        .text_size(px(11.))
                        .text_color(mx.fg_dim)
                        .child(keys),
                )
                .child(div().text_size(px(12.)).text_color(mx.fg_faint).child(label)),
        );
    }
    div()
        .id("empty-pane")
        .size_full()
        .rounded(px(10.))
        .bg(mx.bg)
        .border_1()
        .border_color(mx.border)
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .drag_over::<DraggedTab>(move |s, _, _, _| s.border_color(accent).bg(accent.opacity(0.08)))
        .on_drop(move |d: &DraggedTab, _, cx: &mut App| update_store(&host, cx, |s| s.select(&d.id)))
        .child(
            div()
                .text_size(px(15.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(mx.fg_dim)
                .child(headline(live)),
        )
        .child(
            div()
                .mt(px(10.))
                .w(px(330.))
                .text_center()
                .text_size(px(12.5))
                .line_height(px(19.))
                .text_color(mx.fg_faint)
                .child(DRAG_HINT),
        )
        .child(sheet)
}

pub fn empty_screen(mx: &Mx, host: &Entity<StoreHost>, plugin: &str, cx: &App) -> impl IntoElement {
    let store = &host.read(cx).store;
    let info = store.plugins().and_then(|r| r.get(plugin));
    let name = info.as_ref().map_or_else(|| plugin.to_owned(), |p| p.name().to_owned());
    let glyph = info.as_ref().and_then(|p| p.manifest.as_ref().and_then(|m| m.icon.clone()));
    let dir = info.map(|p| p.dir);
    let home = store.screen_home();
    let host = host.clone();
    div()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(14.))
        .child(crate::rfw_view::plugin_glyph(glyph.as_deref(), dir.as_deref(), 40., mx.fg_faint))
        .child(div().text_size(px(14.)).text_color(mx.fg_dim).child(mx_plugins::rail::empty_screen_text(&name)))
        .children(home.map(|cmd| {
            div()
                .id("screen-home")
                .px(px(14.))
                .py(px(7.))
                .rounded(px(8.))
                .bg(mx.accent)
                .text_color(crate::app::theme::on_color(mx.accent, mx.canvas))
                .text_size(px(13.))
                .font_weight(FontWeight::MEDIUM)
                .cursor_pointer()
                .on_click(move |_, _, cx: &mut App| update_store(&host, cx, |s| s.open_screen_home()))
                .child(cmd.title)
        }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_headline_counts_sessions_left_running() {
        assert_eq!(headline(0), "Nenhum painel aberto");
        assert_eq!(
            headline(1),
            "1 sessão rodando na lateral — clique nela pra trazer de volta"
        );
        assert_eq!(
            headline(4),
            "4 sessões rodando na lateral — clique numa pra trazer de volta"
        );
    }
}
