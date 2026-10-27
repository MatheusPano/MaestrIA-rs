use gpui::{AnyElement, Context, FontWeight, IntoElement, ParentElement, Styled, div, px, relative};

use super::dialog::{SettingsDialog, heading};
use super::model::{Account, SettingsAction, usage_tone};
use crate::chrome::icon::{self, icon};
use crate::chrome::style::{MONO, alpha, hsla, theme};
use crate::chrome::widgets::IconButton;

pub fn page(d: &mut SettingsDialog, cx: &mut Context<SettingsDialog>) -> AnyElement {
    let t = theme(cx);
    let head = heading("Conta & uso", Some("O mesmo que o /usage do Claude mostra, lido da credencial desta máquina"), false, None, cx);
    let reload = || IconButton::new("usage-reload", icon::REFRESH).size(24., 15., 6.).tooltip("Ler de novo").on_click(d.send(SettingsAction::ReloadUsage));
    let body: AnyElement = match d.data.account.clone() {
        Account::Loading => div().flex().flex_col().gap(px(10.))
            .children([180., 260., 220.].map(|w| div().w(px(w)).h(px(10.)).rounded(px(4.)).bg(hsla(t.bg_hover))))
            .into_any_element(),
        Account::Error(problem) => div().flex().items_center().gap(px(10.)).p(px(14.)).rounded(px(9.)).border_1().border_color(hsla(t.border))
            .child(icon(icon::CLOUD_OFF_OUTLINED, 18., hsla(t.fg_dim)))
            .child(div().flex_1().text_size(px(12.)).text_color(hsla(t.fg_dim)).child(problem))
            .child(reload())
            .into_any_element(),
        Account::Ready { org, plan, email, windows, credits, read_at } => {
            let mut col = div().flex().flex_col().gap(px(16.))
                .child(div().flex().flex_col().gap(px(2.))
                    .child(div().text_size(px(12.5)).font_weight(FontWeight::SEMIBOLD).text_color(hsla(t.fg)).child(format!("{org} · {plan}")))
                    .child(div().text_size(px(11.5)).text_color(hsla(t.fg_faint)).child(email)));
            for w in windows {
                let color = usage_tone(w.percent).color(&t);
                let frac = (w.percent / 100.).clamp(0., 1.) as f32;
                col = col.child(div().flex().flex_col().gap(px(5.))
                    .child(div().flex().items_center()
                        .child(div().flex_1().text_size(px(12.5)).text_color(hsla(t.fg)).child(w.label))
                        .child(div().font_family(MONO).font_weight(FontWeight::SEMIBOLD).text_size(px(12.)).text_color(hsla(color)).child(format!("{}%", w.percent.round() as i64))))
                    .child(div().h(px(5.)).rounded(px(2.5)).bg(alpha(color, 0.3)).child(div().h_full().w(relative(frac)).rounded(px(2.5)).bg(hsla(color))))
                    .child(div().text_size(px(11.)).text_color(hsla(t.fg_faint)).child(format!("Reseta em {}", w.resets))));
            }
            if let Some(c) = credits {
                col = col.child(div().text_size(px(11.5)).text_color(hsla(t.fg_dim)).child(c));
            }
            col.child(div().flex().items_center().gap(px(6.)).child(div().text_size(px(11.)).text_color(hsla(t.fg_faint)).child(format!("Lida às {read_at}"))).child(reload()))
                .into_any_element()
        }
    };
    div().flex().flex_col().child(head).child(body).into_any_element()
}
