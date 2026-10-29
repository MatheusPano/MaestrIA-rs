use std::rc::Rc;

use gpui::{App, AppContext, Context, IntoElement, ParentElement, Render, Styled, Window, div, px};
use mx_store::Store;
use mx_sys::updater::INSTALLS_ON_QUIT;

use crate::app::ui;
use crate::chrome::dialog::frame::alert;
use crate::chrome::icon::{self, Icon, icon};
use crate::chrome::style::{cb, hsla, theme};
use crate::chrome::widgets::{Button, ButtonKind};

pub fn title(version: &str) -> String {
    format!("Atualizar pra {version}?")
}

pub fn lines(version: &str, live: usize, installs_on_quit: bool) -> Vec<(Icon, String)> {
    let restart = if live == 0 {
        format!("O app fecha e abre de novo, já na {version}")
    } else {
        format!("O app fecha e abre de novo: as {live} sessões abertas são encerradas, e os painéis voltam como estavam")
    };
    let when = if installs_on_quit {
        (icon::SCHEDULE_ROUNDED, "Sem pressa: ela entra sozinha quando você fechar o app".to_owned())
    } else {
        (icon::LOCK_OUTLINE_ROUNDED, "O sistema vai pedir a sua senha pra instalar o pacote".to_owned())
    };
    vec![(icon::RESTART_ALT_ROUNDED, restart), when]
}

pub struct UpdateConfirm {
    version: String,
    lines: Vec<(Icon, String)>,
    generation: u64,
}

impl UpdateConfirm {
    fn finish(&mut self, go: bool, cx: &mut Context<Self>) {
        ui::close_if(self.generation, cx);
        if go {
            restart(cx);
        }
    }
}

impl Render for UpdateConfirm {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme(cx);
        let me = cx.entity().downgrade();
        let body = div().flex().flex_col().children(self.lines.iter().map(|(glyph, text)| {
            div()
                .flex()
                .items_start()
                .gap(px(10.))
                .py(px(4.))
                .child(icon(*glyph, 16., hsla(t.fg_dim)))
                .child(div().flex_1().text_size(px(13.)).text_color(hsla(t.fg)).child(text.clone()))
        }));
        let actions = vec![
            Button::new("update-later", "Depois", ButtonKind::Text)
                .on_click(cb(me.clone(), |this, _, cx| this.finish(false, cx)))
                .into_any_element(),
            Button::new("update-now", "Reiniciar agora", ButtonKind::Primary)
                .on_click(cb(me.clone(), |this, _, cx| this.finish(true, cx)))
                .into_any_element(),
        ];
        alert(title(&self.version), Some(440.), body, actions, Rc::new(cb(me, |this, _, cx| this.finish(false, cx))), cx)
    }
}

pub fn confirm(cx: &mut App) {
    let store = ui::store(cx);
    let Some(version) = store.app_updater().release().map(|r| r.version.clone()) else { return };
    let live = crate::status_bar::Summary::of(store).sessions;
    let lines = lines(&version, live, INSTALLS_ON_QUIT);
    let generation = ui::upcoming(cx);
    let view = cx.new(|_| UpdateConfirm { version, lines, generation });
    ui::modal(view.into(), cx);
}

fn quit(cx: &mut App) {
    ui::act(cx, Store::relaunch_after_quit);
    cx.quit();
}

fn restart(cx: &mut App) {
    if INSTALLS_ON_QUIT {
        quit(cx);
        return;
    }
    let Some(done) = ui::act(cx, Store::install_update) else { return };
    cx.spawn(async move |cx| {
        if let Ok(Ok(())) = done.await {
            cx.update(quit).ok();
        }
    })
    .detach();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_confirm_counts_the_sessions_that_close() {
        assert_eq!(title("3.1.0"), "Atualizar pra 3.1.0?");
        assert_eq!(lines("3.1.0", 0, false)[0].1, "O app fecha e abre de novo, já na 3.1.0");
        assert_eq!(
            lines("3.1.0", 2, false)[0].1,
            "O app fecha e abre de novo: as 2 sessões abertas são encerradas, e os painéis voltam como estavam"
        );
    }

    #[test]
    fn linux_asks_for_the_password_and_mac_waits_for_quit() {
        assert_eq!(lines("3.1.0", 0, false)[1], (icon::LOCK_OUTLINE_ROUNDED, "O sistema vai pedir a sua senha pra instalar o pacote".into()));
        assert_eq!(lines("3.1.0", 0, true)[1], (icon::SCHEDULE_ROUNDED, "Sem pressa: ela entra sozinha quando você fechar o app".into()));
    }
}
