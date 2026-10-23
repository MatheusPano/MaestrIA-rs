use std::rc::Rc;

use gpui::{AnyElement, Context, FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString, Styled, div, px};

use super::dialog::{SettingsDialog, heading};
use super::model::{SettingsAction, hibernate_label, hibernate_less, hibernate_more, open_panels, remove_launcher_tip, MAX_HIBERNATE, MIN_HIBERNATE};
use crate::chrome::icon::{self, icon};
use crate::chrome::menu::actions::{group_index, launcher_icon};
use crate::chrome::style::{MONO, hsla, theme};
use crate::chrome::widgets::{Button, ButtonKind, IconButton, Stepper, Switch};

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

pub fn sessions(d: &mut SettingsDialog, cx: &mut Context<SettingsDialog>) -> AnyElement {
    let m = d.data.hibernate_minutes;
    let on = m > 0;
    let mut col = div().flex().flex_col().child(heading(
        "Hibernação",
        Some("Uma sessão do Claude parada no prompt custa uns 200MB de RAM pra não fazer nada. Hibernar é desligar o processo e ficar com a conversa: a linha continua na lateral, e o clique nela retoma de onde parou. Também dá pra hibernar uma sessão à mão, pelo menu da linha."),
        false,
        None,
        cx,
    ))
    .child(
        Switch::new("hibernate", "Hibernar sozinho o que está parado fora da tela", on, d.send(SettingsAction::SetHibernateMinutes(if on { 0 } else { 30 })))
            .subtitle("Só as que não estão na tela, sem pendência com você, sem fila pra andar e sem agentes rodando. O que estava rodando dentro dela — um servidor MCP, um dev server que uma ferramenta subiu — vai junto com o processo."),
    );
    if on {
        let less = (m > MIN_HIBERNATE).then(|| Rc::new(d.send(SettingsAction::SetHibernateMinutes(hibernate_less(m)))) as _);
        let more = (m < MAX_HIBERNATE).then(|| Rc::new(d.send(SettingsAction::SetHibernateMinutes(hibernate_more(m)))) as _);
        col = col.child(div().mt(px(14.)).flex().child(Stepper::new("hibernate-after", "Depois de quanto tempo parada", hibernate_label(m), less, more)));
    }
    col.into_any_element()
}
