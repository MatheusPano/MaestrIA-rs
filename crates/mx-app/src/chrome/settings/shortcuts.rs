use gpui::{AnyElement, Context, FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString, StatefulInteractiveElement, Styled, div, px};

use super::dialog::{SettingsDialog, heading};
use super::model::{SettingsAction, ShortcutRow};
use crate::chrome::icon::{self, icon};
use crate::chrome::style::{MONO, hsla, theme};
use crate::chrome::widgets::{Button, ButtonKind, IconButton, tip};

pub const GROUPS: [&str; 4] = ["Sessões", "Navegação", "Exibição", "Janela"];
pub const RECORD_PROMPT: &str = "Pressione a combinação   ⎋ cancela";

fn row(d: &SettingsDialog, r: &ShortcutRow, cx: &mut Context<SettingsDialog>) -> AnyElement {
    let t = theme(cx);
    let recording = d.recording.as_ref().filter(|rec| rec.action == r.action);
    let mut keys = div().flex().flex_wrap().items_center().justify_end().gap(px(6.));
    if let Some(rec) = recording {
        let mut boxed = div().debug_selector(|| "recorder".into()).flex().items_center().gap(px(6.)).h(px(26.)).px(px(10.)).rounded(px(6.)).border_1().border_color(hsla(t.accent))
            .child(div().text_size(px(11.)).text_color(hsla(t.accent)).child(RECORD_PROMPT));
        if let Some(why) = &rec.rejection {
            let sel = why.clone();
            boxed = boxed.child(div().debug_selector(move || format!("rejection:{sel}")).flex().items_center().gap(px(4.))
                .child(icon(icon::BLOCK, 13., hsla(t.yellow)))
                .child(div().text_size(px(11.)).text_color(hsla(t.yellow)).child(why.clone())));
        }
        keys = keys.child(boxed);
    } else {
        let many = r.chords.len() > 1;
        for (id, label) in &r.chords {
            let group: SharedString = format!("cap-{}-{id}", r.action).into();
            let (action, chord) = (r.action.clone(), id.clone());
            let (a2, c2) = (r.action.clone(), id.clone());
            let label_s = label.clone();
            let mut cap = div().id(group.clone()).group(group.clone()).debug_selector(move || format!("keycap:{label_s}"))
                .relative().flex().items_center().h(px(26.)).px(px(9.)).rounded(px(6.)).cursor_pointer()
                .bg(hsla(t.bg)).border_1().border_color(hsla(t.border)).hover(|s| s.border_color(hsla(t.accent)))
                .font_family(MONO).text_size(px(12.)).text_color(hsla(t.fg)).child(label.clone())
                .tooltip(tip("Trocar"))
                .on_click(cx.listener(move |this, _, w, cx| this.record(action.clone(), Some(chord.clone()), w, cx)));
            if many {
                let unbind = d.send(SettingsAction::Unbind { action: a2, chord: c2 });
                cap = cap.child(div().absolute().top(px(-7.)).right(px(-7.)).invisible().group_hover(group, |s| s.visible())
                    .child(IconButton::new(SharedString::from(format!("x-{}-{id}", r.action)), icon::CLOSE_ROUNDED).size(16., 11., 4.).hover_bg(t.bg_active).on_click(unbind)));
            }
            keys = keys.child(cap);
        }
        let action = r.action.clone();
        keys = keys.child(IconButton::new(SharedString::from(format!("add-{}", r.action)), icon::ADD).size(26., 15., 6.).tooltip("Adicionar outra combinação")
            .on_click({
                let me = cx.entity().downgrade();
                move |w, cx| { me.update(cx, |this, cx| this.record(action.clone(), None, w, cx)).ok(); }
            }));
        if r.changed {
            keys = keys.child(IconButton::new(SharedString::from(format!("undo-{}", r.action)), icon::UNDO).size(26., 15., 6.).tooltip("Voltar ao padrão")
                .on_click(d.send(SettingsAction::ResetShortcuts(Some(r.action.clone())))));
        }
    }
    let mut text = div().flex_1().min_w_0().flex().flex_col().gap(px(2.)).child(div().text_size(px(12.5)).text_color(hsla(t.fg)).child(r.label.clone()));
    if let Some(h) = &r.hint {
        text = text.child(div().text_size(px(11.)).text_color(hsla(t.fg_faint)).child(h.clone()));
    }
    div().flex().items_center().gap(px(12.)).py(px(7.)).child(text).child(keys).into_any_element()
}

pub fn page(d: &mut SettingsDialog, cx: &mut Context<SettingsDialog>) -> AnyElement {
    let t = theme(cx);
    let restore = Button::new("shortcuts-restore", "Restaurar padrões", ButtonKind::Text).on_click(d.send(SettingsAction::ResetShortcuts(None))).into_any_element();
    let mut col = div().flex().flex_col().child(heading(
        "Atalhos",
        Some("Clique numa tecla pra trocar, no + pra dar uma segunda combinação à mesma ação. Uma tecla só tem um dono: dar a outra ação tira de quem a tinha."),
        false,
        Some(restore),
        cx,
    ));
    let rows = d.data.shortcuts.clone();
    for g in GROUPS {
        let in_group: Vec<&ShortcutRow> = rows.iter().filter(|r| r.group == g).collect();
        if in_group.is_empty() {
            continue;
        }
        col = col.child(div().mt(px(10.)).mb(px(2.)).text_size(px(10.5)).font_weight(FontWeight::SEMIBOLD).text_color(hsla(t.fg_faint)).child(g));
        for r in in_group {
            col = col.child(row(d, r, cx));
        }
    }
    let fixed = d.data.fixed.iter().map(|(keys, what)| {
        div().flex().items_center().gap(px(12.)).py(px(4.))
            .child(div().w(px(150.)).font_family(MONO).text_size(px(12.)).text_color(hsla(t.fg_dim)).child(keys.clone()))
            .child(div().flex_1().text_size(px(12.)).text_color(hsla(t.fg_faint)).child(what.clone()))
    });
    col.child(
        div().mt(px(18.)).p(px(12.)).rounded(px(8.)).border_1().border_color(hsla(t.border))
            .child(div().mb(px(6.)).text_size(px(10.5)).font_weight(FontWeight::SEMIBOLD).text_color(hsla(t.fg_faint)).child("Fixos"))
            .children(fixed),
    )
    .into_any_element()
}

pub fn fixed_rows(linux: bool) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let paste = if linux { "Ctrl+Shift+V  Ctrl+V  Shift+Insert" } else { "⌘V" };
    out.push((paste.into(), "Colar no painel — imagem inclusa, que o Claude recebe como ^V".into()));
    if linux {
        out.push(("Ctrl+Shift+C".into(), "Copiar a seleção — o Ctrl+C é do processo".into()));
    }
    out.push(("⇧↩".into(), "Quebrar linha no prompt sem enviar".into()));
    out.push(("⌃C".into(), "Do processo, como em qualquer terminal".into()));
    if linux {
        out.push(("Alt+F4".into(), "Do desktop: fechar a janela".into()));
    } else {
        out.push(("⌘Q ⌘W ⌘H ⌘M".into(), "Do macOS: sair, fechar, esconder, minimizar".into()));
    }
    out
}
