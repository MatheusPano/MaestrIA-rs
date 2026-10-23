use gpui::{
    ClipboardItem, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render, StatefulInteractiveElement, Styled, Window,
    div, px,
};

use super::widgets::{Close, barrier, card, text_button, title};
use crate::app::host::StoreHost;
use crate::chrome::dialog::frame::BARRIER;
use crate::chrome::style::{MONO, hsla, theme};

pub const EMPTY_LOG: &str = "Nada ainda";

pub fn log_title(name: &str) -> String {
    format!("Log de {name}")
}

pub fn newest_first(lines: &[String]) -> Vec<String> {
    lines.iter().rev().cloned().collect()
}

pub struct LogDialog {
    host: Entity<StoreHost>,
    plugin: String,
    close: Close,
}

impl LogDialog {
    pub fn new(host: Entity<StoreHost>, plugin: String, close: Close) -> Self {
        LogDialog { host, plugin, close }
    }
}

impl Render for LogDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme(cx);
        let store = &self.host.read(cx).store;
        let registry = store.plugins();
        let name = registry.and_then(|r| r.get(&self.plugin)).map_or_else(|| self.plugin.clone(), |p| p.name().to_owned());
        let lines = registry.map(|r| r.log_of(&self.plugin)).unwrap_or_default();
        let shown = newest_first(&lines);
        let mut list = div().id("plugin-log").h(px(380.)).overflow_y_scroll().px(px(16.)).py(px(8.)).font_family(MONO).text_size(px(11.5));
        if shown.is_empty() {
            list = list.child(div().text_color(hsla(t.fg_faint)).child(EMPTY_LOG));
        }
        for line in shown {
            list = list.child(div().text_color(hsla(t.fg_dim)).child(line));
        }
        let text = lines.join("\n");
        let (close, dismiss) = (self.close.clone(), self.close.clone());
        barrier(BARRIER, move |w, cx| dismiss(w, cx)).flex().items_center().justify_center().child(
            card(&t, 640.)
                .child(div().px(px(24.)).pt(px(18.)).pb(px(8.)).child(title(&t, log_title(&name))))
                .child(div().border_t_1().border_b_1().border_color(hsla(t.border)).child(list))
                .child(
                    div()
                        .px(px(16.))
                        .py(px(12.))
                        .flex()
                        .justify_end()
                        .gap(px(4.))
                        .child(text_button("log-copy", "Copiar", hsla(t.accent), None, move |_, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(text.clone()))
                        }))
                        .child(text_button("log-close", "Fechar", hsla(t.accent), None, move |w, cx| close(w, cx))),
                ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_log_reads_newest_first_and_names_the_plugin() {
        assert_eq!(log_title("Docker"), "Log de Docker");
        let lines = vec!["a".to_owned(), "b".to_owned()];
        assert_eq!(newest_first(&lines), ["b", "a"]);
    }
}
