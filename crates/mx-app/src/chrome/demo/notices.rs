use std::rc::Rc;

use gpui::{AnyView, App, AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled, Window, div, px};
use mx_core::model::NoticeKind;

use crate::chrome::anim::{ConfettiLayer, Pulse, Spinner};
use crate::chrome::anim::reveal::reveal_overlay;
use crate::chrome::notices::{Banner, NoticeCenter, NoticeCenterProps, NoticeView, RowHandlers, ToastStack};
use crate::chrome::style::{hsla, theme};

pub fn notices() -> Vec<NoticeView> {
    let n = |id, kind, title: &str, detail: Option<&str>, read, gone| NoticeView {
        id,
        kind,
        title: title.into(),
        detail: detail.map(Into::into),
        read,
        gone,
        folder: Some("maestria".into()),
        ago: if id == 1 { "agora".into() } else { format!("{}min", id * 3) },
    };
    vec![
        n(1, NoticeKind::Question, "Revisar o PR", Some("posso apagar a branch?"), false, false),
        n(2, NoticeKind::Permission, "Migrar o banco", None, false, false),
        n(3, NoticeKind::Finished, "Plugin do ArgoCD", None, true, false),
        n(4, NoticeKind::Suggested, "Lint", Some("Remover código morto"), true, true),
    ]
}

struct NoticesScene {
    toasts: Entity<ToastStack>,
    center: Entity<NoticeCenter>,
    confetti: Entity<ConfettiLayer>,
}

impl Render for NoticesScene {
    fn render(&mut self, _w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme(cx);
        div()
            .size_full()
            .relative()
            .child(div().absolute().top(px(16.)).left(px(16.)).flex().flex_col().gap(px(16.))
                .child(Banner::new("Grupo \"3 terminais\" agora é três terminais", Rc::new(|_, _| {})))
                .child(div().flex().gap(px(12.)).items_center()
                    .child(Spinner::new(14., t.accent)).child(Pulse::new(14., t.purple))
                    .child(div().relative().w(px(240.)).h(px(40.)).rounded(px(9.)).bg(hsla(t.bg_sidebar)).child(reveal_overlay(&t))
                        .child(div().p(px(11.)).text_size(px(13.)).child("linha revelada")))))
            .child(div().absolute().top(px(16.)).right(px(16.)).child(self.center.clone()))
            .child(div().absolute().bottom(px(16.)).right(px(16.)).child(self.toasts.clone()))
            .child(self.confetti.clone())
    }
}

pub fn scene(_component: &str, window: &mut Window, cx: &mut App) -> AnyView {
    let handlers = || RowHandlers {
        on_open: Rc::new(|id, _, _| log::info!("open {id}")),
        on_close: Rc::new(|id, _, _| log::info!("close {id}")),
        on_hover: None,
    };
    let toasts = cx.new(|cx| {
        let mut s = ToastStack::new(handlers());
        s.set(notices().into_iter().take(2).collect(), cx);
        s
    });
    let center = cx.new(|cx| {
        let mut c = NoticeCenter::new(NoticeCenterProps {
            on_toggle_dnd: Rc::new(|_, _| {}),
            on_read_all: Rc::new(|_, _| {}),
            on_clear_all: Rc::new(|_| log::info!("cleared")),
            on_close: Rc::new(|_, _| {}),
            on_open: Rc::new(|_, _, _| {}),
            on_dismiss: Rc::new(|_, _, _| {}),
        });
        c.set(notices(), false, cx);
        c
    });
    let confetti = cx.new(|_| ConfettiLayer::new());
    if std::env::var("MAESTRIA_DEMO_CONFETTI").is_ok() {
        let size = window.viewport_size();
        let layer = confetti.clone();
        window
            .spawn(cx, async move |cx| {
                loop {
                    cx.update(|window, cx| {
                        layer.update(cx, |c, cx| {
                            c.fire(cx);
                            c.puff(gpui::point(size.width / 2., size.height / 2.), window, cx);
                        })
                    })
                    .ok();
                    cx.background_executor().timer(std::time::Duration::from_millis(1500)).await;
                }
            })
            .detach();
    }
    cx.new(|_| NoticesScene { toasts, center, confetti }).into()
}
