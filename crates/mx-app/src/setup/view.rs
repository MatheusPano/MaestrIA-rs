use gpui::{
    AnyElement, ClickEvent, Context, ElementId, FontWeight, Hsla, InteractiveElement,
    IntoElement, KeyDownEvent, ParentElement, Render, SharedString, StatefulInteractiveElement,
    Styled, Window, div, prelude::FluentBuilder, px,
};
use mx_claude::setup::{self, SetupSection, SetupSlot};
use mx_theme::Theme;

use super::SetupView;
use super::state::{StateTone, save_tooltip};
pub use crate::chrome::style::hsla;
use crate::chrome::icon::{self, Icon, icon};
use crate::chrome::style::{MONO, alpha};
use crate::chrome::widgets::tip;

pub const INDEX_WIDTH: f32 = 232.;
pub const HEADER_HEIGHT: f32 = 42.;

const WELCOME: &str = "Escolha um arquivo à esquerda pra editar. O que está apagado ainda não existe: abrir mostra um modelo, e salvar cria. O ? de cada prateleira explica o que ela é, com exemplo.";

type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut gpui::App)>;

#[derive(Clone, Copy)]
enum Glyph {
    Icon(Icon),
    Save,
}

fn floppy(size: f32, color: Hsla) -> AnyElement {
    let s = size * 0.8;
    div()
        .flex_none()
        .size(px(size))
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .relative()
                .size(px(s))
                .rounded(px(1.5))
                .border(px(1.5))
                .border_color(color)
                .child(div().absolute().top_0().left(px(s * 0.22)).w(px(s * 0.42)).h(px(s * 0.3)).bg(color))
                .child(div().absolute().bottom(px(s * 0.1)).left(px(s * 0.18)).w(px(s * 0.45)).h(px(s * 0.28)).rounded(px(1.)).bg(color)),
        )
        .into_any_element()
}

fn action(
    id: impl Into<ElementId>,
    glyph: Glyph,
    tooltip: impl Into<SharedString>,
    color: Hsla,
    t: &Theme,
    on_click: Option<ClickHandler>,
) -> AnyElement {
    let tooltip = tooltip.into();
    let selector = tooltip.clone();
    let enabled = on_click.is_some();
    let color = if enabled { color } else { alpha(t.fg_faint, 0.5) };
    let glyph = match glyph {
        Glyph::Icon(i) => icon(i, 15., color).into_any_element(),
        Glyph::Save => floppy(15., color),
    };
    let hover = hsla(t.bg_hover);
    div()
        .id(id.into())
        .debug_selector(move || format!("setup:{selector}"))
        .flex_none()
        .size(px(26.))
        .rounded(px(6.))
        .flex()
        .items_center()
        .justify_center()
        .tooltip(tip(tooltip))
        .child(glyph)
        .when_some(on_click, move |d, f| {
            d.cursor_pointer().hover(move |s| s.bg(hover)).on_click(move |ev, window, cx| {
                cx.stop_propagation();
                f(ev, window, cx)
            })
        })
        .into_any_element()
}

impl SetupView {
    fn click(
        &self,
        cx: &mut Context<Self>,
        f: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> Option<ClickHandler> {
        let me = cx.entity().downgrade();
        Some(Box::new(move |_, window, cx| {
            me.update(cx, |this, cx| f(this, window, cx)).ok();
        }))
    }

    fn help_button(&self, section: SetupSection, size: f32, cx: &mut Context<Self>) -> AnyElement {
        let label = section.label();
        let f = self.click(cx, move |this, w, cx| this.explain(section, w, cx));
        let faint = hsla(self.theme.fg_faint);
        div()
            .id(SharedString::from(format!("help:{label}:{size}")))
            .debug_selector(move || format!("help:{label}"))
            .flex_none()
            .size(px(22.))
            .rounded(px(6.))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .hover({
                let h = hsla(self.theme.bg_hover);
                move |s| s.bg(h)
            })
            .tooltip(tip(format!("O que é {label}?")))
            .on_click(move |ev, w, cx| {
                cx.stop_propagation();
                if let Some(f) = &f {
                    f(ev, w, cx);
                }
            })
            .child(icon(icon::HELP_OUTLINE, size, faint))
            .into_any_element()
    }

    fn header(&self) -> AnyElement {
        let t = self.theme.clone();
        let root = self.setup.root.clone();
        let mut buttons: Vec<AnyElement> = Vec::new();
        if let Some(reveal) = self.actions.reveal.clone() {
            let root = root.clone();
            buttons.push(action("setup-reveal", Glyph::Icon(icon::FOLDER_OPEN), "Mostrar a pasta no Finder", hsla(t.fg_dim), &t, Some(Box::new(move |_, w, cx| reveal(root.clone(), w, cx)))));
        }
        if let Some(open) = self.actions.open_in_editor.clone() {
            let root = root.clone();
            buttons.push(action("setup-open-root", Glyph::Icon(icon::EDIT_OUTLINED), "Abrir a pasta no vscode", hsla(t.fg_dim), &t, Some(Box::new(move |_, w, cx| open(root.clone(), w, cx)))));
        }
        if let Some(pin) = self.actions.toggle_pin.clone() {
            let (glyph, color, tooltip) = if self.pinned { (icon::PUSH_PIN, t.accent, "Solto") } else { (icon::PUSH_PIN_OUTLINED, t.fg_faint, "Prender") };
            buttons.push(action("setup-pin", Glyph::Icon(glyph), tooltip, hsla(color), &t, Some(Box::new(move |_, w, cx| pin(w, cx)))));
        }
        if let Some(close) = self.actions.close.clone() {
            buttons.push(action("setup-close", Glyph::Icon(icon::CLOSE), "Fechar esta configuração", hsla(t.fg_dim), &t, Some(Box::new(move |_, w, cx| close(w, cx)))));
        }
        div()
            .h(px(HEADER_HEIGHT))
            .flex_none()
            .flex()
            .items_center()
            .pl(px(12.))
            .pr(px(8.))
            .bg(hsla(t.bg_sidebar))
            .border_b_1()
            .border_color(hsla(t.border))
            .child(icon(icon::TUNE, 17., hsla(t.fg_dim)))
            .child(div().w(px(9.)))
            .child(div().flex_shrink().min_w_0().truncate().text_size(px(15.)).font_weight(FontWeight::BOLD).text_color(hsla(t.fg)).child(self.title()))
            .child(div().ml(px(8.)).flex_1().min_w_0().truncate().font_family(MONO).text_size(px(11.)).text_color(hsla(t.fg_faint)).child(root))
            .child(div().flex_none().flex().items_center().children(buttons))
            .into_any_element()
    }

    fn slot_row(&self, slot: &SetupSlot, cx: &mut Context<Self>) -> AnyElement {
        let t = &self.theme;
        let selected = self.setup.selected.as_deref() == Some(slot.rel.as_str());
        let (glyph, dot) = if slot.exists {
            (icon::CIRCLE, if selected { t.accent } else { t.fg_dim })
        } else {
            (icon::CIRCLE_OUTLINED, t.fg_faint)
        };
        let rel = slot.rel.clone();
        let label = slot.label.clone();
        let hover = hsla(t.bg_hover);
        div()
            .id(SharedString::from(format!("slot:{rel}")))
            .debug_selector({
                let rel = rel.clone();
                move || format!("slot:{rel}")
            })
            .flex()
            .items_center()
            .gap(px(9.))
            .px(px(8.))
            .py(px(6.))
            .rounded(px(6.))
            .cursor_pointer()
            .when(selected, |d| d.bg(hsla(t.bg_active)))
            .when(!selected, move |d| d.hover(move |s| s.bg(hover)))
            .on_click(cx.listener(move |this, _, w, cx| this.select(rel.clone(), w, cx)))
            .child(icon(glyph, 7., hsla(dot)))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .font_family(MONO)
                    .text_size(px(11.5))
                    .text_color(hsla(if slot.exists { t.fg } else { t.fg_faint }))
                    .when(!slot.exists, |d| d.italic())
                    .child(label),
            )
            .into_any_element()
    }

    fn index(&self, cx: &mut Context<Self>) -> AnyElement {
        let t = self.theme.clone();
        let mut list = div().flex().flex_col().pt(px(10.)).px(px(8.)).pb(px(16.));
        for section in SetupSection::ALL {
            let mut head = div()
                .flex()
                .items_center()
                .pl(px(8.))
                .pt(px(6.))
                .pr(px(2.))
                .pb(px(2.))
                .child(div().flex_1().text_size(px(10.5)).font_weight(FontWeight::SEMIBOLD).text_color(hsla(t.fg_faint)).child(section.label()))
                .child(self.help_button(section, 12., cx));
            if section.open() {
                let tooltip = format!("Nova {}", section.noun());
                head = head.child(action(
                    SharedString::from(format!("add:{}", section.label())),
                    Glyph::Icon(icon::ADD),
                    tooltip,
                    hsla(t.fg_faint),
                    &t,
                    self.click(cx, move |this, w, cx| this.add(section, w, cx)),
                ));
            }
            list = list.child(head);
            let mut any = false;
            for slot in self.slots.iter().filter(|s| s.section == section) {
                any = true;
                list = list.child(self.slot_row(slot, cx));
            }
            if section.open() && !any {
                let hover = hsla(t.bg_hover);
                list = list.child(
                    div()
                        .id(SharedString::from(format!("first:{}", section.label())))
                        .debug_selector(move || format!("first:{}", section.label()))
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .px(px(8.))
                        .py(px(6.))
                        .rounded(px(6.))
                        .cursor_pointer()
                        .hover(move |s| s.bg(hover))
                        .on_click(cx.listener(move |this, _, w, cx| this.add(section, w, cx)))
                        .child(icon(icon::ADD_CIRCLE_OUTLINE, 12., hsla(t.fg_faint)))
                        .child(div().text_size(px(11.5)).text_color(hsla(t.fg_faint)).child(format!("Criar a primeira {}…", section.noun()))),
                );
            }
            list = list.child(div().h(px(8.)));
        }
        div()
            .id("setup-index")
            .w(px(INDEX_WIDTH))
            .flex_none()
            .h_full()
            .bg(hsla(t.bg_sidebar))
            .overflow_y_scroll()
            .track_scroll(&self.list_scroll)
            .child(list)
            .into_any_element()
    }

    fn editor_head(&self, slot: &SetupSlot, cx: &mut Context<Self>) -> AnyElement {
        let t = self.theme.clone();
        let text = self.text(cx);
        let (state, tone) = self.doc.state(slot.exists, &text);
        let state_color = match tone {
            StateTone::Warn => t.yellow,
            StateTone::Faint => t.fg_faint,
        };
        let can_save = self.doc.can_save(slot.exists, &text);
        let mut row = div()
            .flex()
            .items_center()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .debug_selector(|| "setup-state".into())
                    .child(div().flex_none().font_family(MONO).text_size(px(12.)).text_color(hsla(t.fg)).child(slot.rel.clone()))
                    .child(div().text_size(px(11.)).text_color(hsla(state_color)).child(format!("  ·  {state}")))
                    .flex()
                    .items_baseline(),
            )
            .child(self.help_button(slot.section, 13., cx))
            .child(div().w(px(2.)))
            .child(action(
                "setup-save",
                Glyph::Save,
                save_tooltip(),
                hsla(if can_save { t.accent } else { t.fg_dim }),
                &t,
                can_save.then(|| self.click(cx, |this, w, cx| this.save(w, cx))).flatten(),
            ))
            .child(action(
                "setup-revert",
                Glyph::Icon(icon::HISTORY_OUTLINED),
                "Voltar ao que está no disco",
                hsla(t.fg_dim),
                &t,
                self.doc.can_revert(&text).then(|| self.click(cx, |this, w, cx| this.revert(w, cx))).flatten(),
            ));
        if slot.exists
            && let Some(open) = self.actions.open_in_editor.clone()
        {
            let path = setup::path_of(&self.setup.root, &slot.rel);
            row = row.child(action("setup-open", Glyph::Icon(icon::EDIT_OUTLINED), "Abrir no vscode", hsla(t.fg_dim), &t, Some(Box::new(move |_, w, cx| open(path.clone(), w, cx)))));
        }
        row = row.child(action(
            "setup-delete",
            Glyph::Icon(icon::DELETE_OUTLINE),
            "Apagar o arquivo",
            hsla(t.red),
            &t,
            slot.exists.then(|| self.click(cx, |this, _, cx| this.delete(cx))).flatten(),
        ));
        div()
            .flex_none()
            .pl(px(18.))
            .pt(px(12.))
            .pr(px(10.))
            .pb(px(10.))
            .border_b_1()
            .border_color(hsla(t.border))
            .child(row)
            .child(div().mt(px(4.)).text_size(px(11.5)).line_height(px(16.)).text_color(hsla(t.fg_dim)).child(slot.about.clone()))
            .into_any_element()
    }

    fn welcome(&self, cx: &mut Context<Self>) -> AnyElement {
        let t = self.theme.clone();
        let mut col = div()
            .w_full()
            .max_w(px(620.))
            .flex()
            .flex_col()
            .child(div().text_size(px(17.)).font_weight(FontWeight::SEMIBOLD).text_color(hsla(t.fg)).child(format!("O que o Claude lê em {}", self.setup.name())))
            .child(div().mt(px(6.)).text_size(px(12.5)).line_height(px(19.)).text_color(hsla(t.fg_dim)).child(WELCOME))
            .child(div().h(px(22.)));
        for section in SetupSection::ALL {
            col = col
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(4.))
                        .child(div().text_size(px(12.5)).font_weight(FontWeight::SEMIBOLD).text_color(hsla(t.fg)).child(section.label()))
                        .child(self.help_button(section, 13., cx)),
                )
                .child(div().mt(px(3.)).mb(px(14.)).text_size(px(12.)).line_height(px(18.)).text_color(hsla(t.fg_dim)).child(section.about()));
        }
        div()
            .id("setup-welcome")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .track_scroll(&self.welcome_scroll)
            .child(div().w_full().pt(px(24.)).px(px(26.)).pb(px(40.)).flex().justify_center().child(col))
            .into_any_element()
    }

    fn root_key(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.overlay.is_some() {
            self.overlay_key(ev, window, cx);
            return;
        }
        let m = ev.keystroke.modifiers;
        if ev.keystroke.key == "s" && (m.control || m.platform) && self.slot().is_some() {
            self.save(window, cx);
            cx.stop_propagation();
        }
    }
}

impl Render for SetupView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme.clone();
        let surface = match self.slot() {
            None => self.welcome(cx),
            Some(slot) => div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(self.editor_head(&slot, cx))
                .child(div().flex_1().min_h_0().child(self.editor.clone()))
                .into_any_element(),
        };
        let header = self.header.then(|| self.header());
        let overlay = self.render_overlay(window, cx);
        div()
            .id("setup-pane")
            .debug_selector(|| "setup-pane".into())
            .track_focus(&self.focus)
            .key_context("SetupPane")
            .on_key_down(cx.listener(Self::root_key))
            .size_full()
            .flex()
            .flex_col()
            .bg(hsla(t.bg))
            .text_color(hsla(t.fg))
            .children(header)
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_row()
                    .child(self.index(cx))
                    .child(div().w(px(1.)).h_full().bg(hsla(t.border)))
                    .child(div().flex_1().min_w_0().h_full().flex().flex_col().child(surface)),
            )
            .children(overlay)
    }
}
