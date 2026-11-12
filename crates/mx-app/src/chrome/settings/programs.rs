use std::rc::Rc;

use gpui::{AnyElement, Context, Div, FontWeight, Hsla, InteractiveElement, IntoElement, ParentElement, SharedString, StatefulInteractiveElement, Styled, div, prelude::FluentBuilder, px};
use mx_core::persist::PaneOpen;

use super::dialog::{SettingsDialog, heading};
use super::model::{SettingsAction, hibernate_label, hibernate_less, hibernate_more, open_panels, remove_launcher_tip, MAX_HIBERNATE, MIN_HIBERNATE};
use crate::chrome::icon::{self, icon};
use crate::chrome::menu::actions::{group_index, launcher_icon};
use crate::chrome::style::{MONO, hsla, theme};
use crate::chrome::widgets::{Button, ButtonKind, IconButton, Stepper, Toggle};

pub const PROGRAMS_EMPTY: &str = "Nenhum ainda. Um `btop` aqui é um painel de monitor a um clique de distância; um `npm run dev` é o servidor do projeto sempre no mesmo lugar.";

pub fn page(d: &mut SettingsDialog, cx: &mut Context<SettingsDialog>) -> AnyElement {
    let t = theme(cx);
    let mut col = div().flex().flex_col().child(heading(
        "Programas",
        Some("Um painel que já abre dentro de um comando — é o que a sessão do Claude sempre foi, com o comando vindo daqui. Cada um vira uma linha no + da lateral, e volta rodando quando o app reabre."),
        false,
        None,
        cx,
    ));
    if d.data.launchers.is_empty() {
        col = col.child(div().p(px(14.)).rounded(px(9.)).border_1().border_color(hsla(t.border)).text_size(px(12.)).line_height(px(17.)).text_color(hsla(t.fg_faint)).child(PROGRAMS_EMPTY));
    }
    for l in d.data.launchers.clone() {
        let color = t.group_tints()[group_index(&l.id)];
        let mut row = div().id(SharedString::from(format!("launcher-{}", l.id))).flex().items_center().gap(px(12.)).px(px(10.)).py(px(8.)).rounded(px(9.)).hover(|s| s.bg(hsla(t.bg_hover)))
            .child(icon(launcher_icon(l.icon), 17., hsla(color)))
            .child(div().flex_1().min_w_0().flex().flex_col().gap(px(2.))
                .child(div().text_size(px(12.5)).font_weight(FontWeight::SEMIBOLD).text_color(hsla(t.fg)).child(l.name.clone()))
                .child(div().font_family(MONO).text_size(px(11.5)).text_color(hsla(t.fg_faint)).child(l.command.clone())));
        if l.open > 0 {
            row = row.child(div().text_size(px(11.)).text_color(hsla(t.fg_faint)).child(open_panels(l.open)));
        }
        row = row
            .child(IconButton::new(SharedString::from(format!("edit-{}", l.id)), icon::EDIT_OUTLINED).size(26., 15., 6.).tooltip("Editar").on_click(d.send(SettingsAction::EditLauncher(l.id.clone()))))
            .child(IconButton::new(SharedString::from(format!("rm-{}", l.id)), icon::CLOSE).size(26., 15., 6.).tooltip(remove_launcher_tip(l.open)).on_click(d.send(SettingsAction::RemoveLauncher(l.id.clone()))));
        col = col.child(row);
    }
    col.child(div().mt(px(12.)).flex().child(Button::new("new-launcher", "Novo programa…", ButtonKind::Text).leading(icon::ADD).on_click(d.send(SettingsAction::NewLauncher))))
        .into_any_element()
}

pub fn keep_on_quit_hint(keep: bool) -> &'static str {
    if keep {
        "Um Claude no meio de uma tarefa continua trabalhando. Ao abrir de novo, cada painel volta como estiver: ainda rodando, ou terminado com a tela do que fez."
    } else {
        "Fechar encerra todos os terminais. Ao abrir de novo, os painéis voltam e o Claude retoma a conversa com --resume."
    }
}

pub fn pane_open_label(mode: PaneOpen) -> &'static str {
    match mode {
        PaneOpen::Replace => "Substituir o painel em foco",
        PaneOpen::Beside => "Abrir ao lado",
        PaneOpen::Below => "Abrir embaixo",
        PaneOpen::Auto => "Automático",
    }
}

fn pane_sketch(mode: PaneOpen, old: Hsla, new: Hsla) -> Div {
    let cell = |c: Hsla| div().flex_1().rounded(px(2.)).bg(c);
    let frame = div().w(px(56.)).h(px(36.)).flex().gap(px(3.));
    match mode {
        PaneOpen::Replace => frame.child(cell(new)),
        PaneOpen::Beside => frame.flex_row().child(cell(old)).child(cell(new)),
        PaneOpen::Below => frame.flex_col().child(cell(old)).child(cell(new)),
        PaneOpen::Auto => frame
            .flex_row()
            .child(cell(old))
            .child(div().flex_1().flex().flex_col().gap(px(3.)).child(cell(new.opacity(0.55))).child(cell(new.opacity(0.55)))),
    }
}

fn pane_open_section(d: &SettingsDialog, cx: &mut Context<SettingsDialog>) -> AnyElement {
    let t = theme(cx);
    let current = d.data.pane_open;
    let options = PaneOpen::ALL.into_iter().map(|mode| {
        let on = mode == current;
        let send = d.send(SettingsAction::SetPaneOpen(mode));
        let sketch = div().flex().justify_center().py(px(8.)).rounded(px(5.)).bg(hsla(t.canvas))
            .child(pane_sketch(mode, hsla(t.fg_faint).opacity(0.35), hsla(t.accent)));
        div().id(SharedString::from(format!("pane-open-{}", mode.key()))).debug_selector(move || format!("pane-open:{}", mode.key()))
            .flex_1().min_w_0().flex().flex_col().gap(px(6.)).p(px(6.)).rounded(px(8.)).cursor_pointer()
            .border_1().border_color(hsla(if on { t.accent } else { t.border })).hover(|s| s.bg(hsla(t.bg_hover)))
            .child(sketch)
            .child(div().px(px(2.)).text_size(px(12.)).text_color(hsla(t.fg)).child(pane_open_label(mode)))
            .when(mode == PaneOpen::Auto, |c| c.child(div().px(px(2.)).text_size(px(11.)).line_height(px(15.)).text_color(hsla(t.fg_faint)).child("Ao lado ou embaixo, pelo formato do painel")))
            .on_click(move |_, w, cx| send(w, cx))
    });
    div().flex().flex_col().mb(px(24.))
        .child(heading("Ao abrir uma sessão", Some("O que acontece com a tela quando você abre uma sessão pela lateral, por um atalho ou cria uma nova. Se não couber mais um painel, ela substitui o painel em foco."), false, None, cx))
        .child(div().flex().gap(px(8.)).children(options))
        .into_any_element()
}

fn focus_follows_mouse_section(d: &SettingsDialog, cx: &mut Context<SettingsDialog>) -> AnyElement {
    let on = d.data.focus_follows_mouse;
    div().flex().flex_col().mb(px(24.))
        .child(heading("Foco", None, false, None, cx))
        .child(
            Toggle::new("focus-follows-mouse", "Foco segue o mouse", on, d.send(SettingsAction::SetFocusFollowsMouse(!on)))
                .subtitle("Passar o mouse por cima de um painel já o deixa em foco, sem precisar clicar — como no Hyprland."),
        )
        .into_any_element()
}

pub fn sessions(d: &mut SettingsDialog, cx: &mut Context<SettingsDialog>) -> AnyElement {
    let m = d.data.hibernate_minutes;
    let on = m > 0;
    let mut col = div().flex().flex_col().child(pane_open_section(d, cx)).child(focus_follows_mouse_section(d, cx)).child(heading(
        "Hibernação",
        Some("Uma sessão do Claude parada no prompt custa uns 200MB de RAM pra não fazer nada. Hibernar é desligar o processo e ficar com a conversa: a linha continua na lateral, e o clique nela retoma de onde parou. Também dá pra hibernar uma sessão à mão, pelo menu da linha."),
        false,
        None,
        cx,
    ))
    .child(
        Toggle::new("hibernate", "Hibernar sozinho o que está parado fora da tela", on, d.send(SettingsAction::SetHibernateMinutes(if on { 0 } else { 30 })))
            .subtitle("Só as que não estão na tela, sem pendência com você, sem fila pra andar e sem agentes rodando. O que estava rodando dentro dela — um servidor MCP, um dev server que uma ferramenta subiu — vai junto com o processo."),
    );
    if on {
        let less = (m > MIN_HIBERNATE).then(|| Rc::new(d.send(SettingsAction::SetHibernateMinutes(hibernate_less(m)))) as _);
        let more = (m < MAX_HIBERNATE).then(|| Rc::new(d.send(SettingsAction::SetHibernateMinutes(hibernate_more(m)))) as _);
        col = col.child(div().mt(px(14.)).flex().child(Stepper::new("hibernate-after", "Depois de quanto tempo parada", hibernate_label(m), less, more)));
    }
    let keep = d.data.keep_on_quit;
    col.child(div().mt(px(24.)).child(heading(
        "Ao fechar o app",
        Some("Os terminais rodam num processo à parte, então fechar o app não precisa encerrar o que está rodando."),
        false,
        None,
        cx,
    )))
    .child(
        Toggle::new("keep-on-quit", "Deixar as sessões rodando quando o app fecha", keep, d.send(SettingsAction::SetKeepOnQuit(!keep)))
            .subtitle(keep_on_quit_hint(keep)),
    )
    .into_any_element()
}
