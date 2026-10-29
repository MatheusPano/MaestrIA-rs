use gpui::{AnyElement, Context, InteractiveElement, IntoElement, ParentElement, SharedString, StatefulInteractiveElement, Styled, Window, div, px};
use mx_theme::Typography;

use super::dialog::{SettingsDialog, heading};
use super::gallery::gallery;
use super::model::{SettingsAction, line_label, size_label};
use crate::chrome::style::{hsla, theme};
use crate::chrome::widgets::{Button, ButtonKind, Stepper, Toggle};

const ALWAYS: fn(&str) -> bool = |_| true;

fn typo(t: &Typography, mono: Option<&str>, size: Option<f64>, line: Option<f64>) -> SettingsAction {
    SettingsAction::SetTypography(t.with(mono, size, line, &ALWAYS))
}

pub fn page(d: &mut SettingsDialog, _window: &mut Window, cx: &mut Context<SettingsDialog>) -> AnyElement {
    let t = theme(cx);
    let data = &d.data;
    let themes = div().flex().flex_col().mb(px(28.))
        .child(heading("Tema", Some("A janela atrás é o preview."), false, None, cx))
        .child(gallery(d, &t));
    let ty = data.typography.clone();
    let restore = (ty != Typography::standard()).then(|| {
        Button::new("typo-restore", "Restaurar padrões", ButtonKind::Text)
            .on_click(d.send(SettingsAction::SetTypography(Typography::standard())))
            .into_any_element()
    });
    let term = t.terminal();
    let fg = term.foreground;
    let c = |rgb: mx_theme::Rgb| hsla(rgb.into());
    let size = ty.size() as f32;
    let line = (ty.size() * ty.line()) as f32;
    let run = |text: &str, color| div().text_color(color).child(text.to_owned());
    let sample = div().p(px(14.)).rounded(px(8.)).bg(c(term.background)).font_family(SharedString::from(ty.mono().to_owned())).text_size(px(size)).line_height(px(line)).whitespace_nowrap()
        .child(div().flex().child(run("$ ", c(term.ansi[8]))).child(run("git", c(term.ansi[2]))).child(run(" status --short", c(fg))))
        .child(div().flex().child(run(" M ", c(term.ansi[3]))).child(run("lib/ui/settings.dart", c(fg))))
        .child(div().flex().child(run("?? Il1 O0 — {} => (i:M)", c(term.ansi[8]))));
    let chips = data.faces.iter().map(|f| {
        let on = f.family == ty.mono();
        let mut chip = div().id(SharedString::from(format!("face-{}", f.family))).debug_selector(move || format!("face:{}", f.family)).px(px(10.)).py(px(7.)).rounded(px(7.)).cursor_pointer()
            .border_1().border_color(hsla(if on { t.accent } else { t.border })).hover(|s| s.bg(hsla(t.bg_hover)))
            .flex().flex_col().gap(px(1.))
            .child(div().font_family(f.family).text_size(px(12.)).text_color(hsla(t.fg)).child(f.family));
        if let Some(n) = f.note {
            chip = chip.child(div().text_size(px(10.5)).text_color(hsla(t.fg_faint)).child(n));
        }
        chip.on_click({
            let f2 = d.send(typo(&ty, Some(f.family), None, None));
            move |_, w, cx| f2(w, cx)
        })
    });
    let step = |v: f64, lo: f64, hi: f64| (v >= lo && v <= hi).then_some(v);
    let less_size = step(ty.size() - 1., 9., 24.).map(|s| std::rc::Rc::new(d.send(typo(&ty, None, Some(s), None))) as _);
    let more_size = step(ty.size() + 1., 9., 24.).map(|s| std::rc::Rc::new(d.send(typo(&ty, None, Some(s), None))) as _);
    let less_line = step((ty.line() * 10. - 1.).round() / 10., 1., 1.8).map(|l| std::rc::Rc::new(d.send(typo(&ty, None, None, Some(l)))) as _);
    let more_line = step((ty.line() * 10. + 1.).round() / 10., 1., 1.8).map(|l| std::rc::Rc::new(d.send(typo(&ty, None, None, Some(l)))) as _);
    let (zin, zout) = &data.zoom_keys;
    let typography = div().flex().flex_col().gap(px(12.)).mb(px(28.))
        .child(heading("Tipografia do pty", Some(&format!("Vale pra todo painel. {zin} e {zout} mexem em um só, a partir daqui — e o painel lembra o que você deixou.")), false, restore, cx))
        .child(sample)
        .child(div().flex().flex_wrap().gap(px(6.)).children(chips))
        .child(div().text_size(px(11.)).text_color(hsla(t.fg_faint)).child("Só as que existem nesta máquina; a Hack vem no app"))
        .child(div().flex().gap(px(24.))
            .child(Stepper::new("corpo", "Corpo", size_label(&ty), less_size, more_size))
            .child(Stepper::new("entrelinha", "Entrelinha", line_label(&ty), less_line, more_line)));
    let top = data.status_bar_top;
    let bar = div().flex().flex_col()
        .child(heading("Barra de status", Some("A faixa com o resumo das sessões e o sino. O sino, a lista dele e os cartões de aviso ficam do mesmo lado que ela."), false, None, cx))
        .child(Toggle::new("bar-top", "Barra de status em cima", top, d.send(SettingsAction::SetStatusBarTop(!top))).subtitle("No topo da janela, em vez de no pé"));
    div().flex().flex_col().child(themes).child(typography).child(bar).into_any_element()
}
