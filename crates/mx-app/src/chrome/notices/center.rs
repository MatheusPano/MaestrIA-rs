use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{
    AnyElement, App, AppContext, Context, FontWeight, InteractiveElement, IntoElement, ParentElement, Render,
    StatefulInteractiveElement, Styled, Window, div, px,
};

use super::anim_off;
use super::row::{NoticeView, RowHandlers, notice_row};
use super::toasts::TOAST_WIDTH;
use crate::chrome::anim::ease::{ease_in_cubic, ease_in_out, interval};
use crate::chrome::icon;
use crate::chrome::style::{Callback, Handler, alpha, hsla, shadow, theme};
use crate::chrome::widgets::IconButton;

pub const ROW_OUT: Duration = Duration::from_millis(280);
pub const STAGGER: Duration = Duration::from_millis(35);
pub const SETTLE: Duration = Duration::from_millis(220);
pub const EMPTY: &str = "Nenhuma notificação — quando um painel perguntar algo ou terminar, aparece aqui";

pub struct NoticeCenterProps {
    pub on_toggle_dnd: Callback,
    pub on_read_all: Callback,
    pub on_clear_all: Rc<dyn Fn(&mut App)>,
    pub on_close: Callback,
    pub on_open: Handler<u64>,
    pub on_dismiss: Handler<u64>,
}

pub struct NoticeCenter {
    notices: Vec<NoticeView>,
    dnd: bool,
    hovered: Option<u64>,
    leaving: HashMap<u64, Instant>,
    clearing: Option<Instant>,
    props: Rc<NoticeCenterProps>,
    born: Instant,
}

pub fn cascade_total(n: usize) -> Duration {
    STAGGER * n.min(8) as u32 + ROW_OUT + SETTLE
}

impl NoticeCenter {
    pub fn new(props: NoticeCenterProps) -> Self {
        Self {
            notices: Vec::new(),
            dnd: false,
            hovered: None,
            leaving: HashMap::new(),
            clearing: None,
            props: Rc::new(props),
            born: Instant::now(),
        }
    }

    pub fn set(&mut self, notices: Vec<NoticeView>, dnd: bool, cx: &mut Context<Self>) {
        self.notices = notices;
        self.dnd = dnd;
        cx.notify();
    }

    pub fn len(&self) -> usize {
        self.notices.len()
    }

    pub fn clearing(&self) -> bool {
        self.clearing.is_some()
    }

    pub fn clear_all(&mut self, cx: &mut Context<Self>) {
        if self.notices.is_empty() || self.clearing.is_some() {
            return;
        }
        self.clearing = Some(Instant::now());
        let total = if anim_off() { Duration::ZERO } else { cascade_total(self.notices.len()) };
        let clear = self.props.on_clear_all.clone();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(total).await;
            cx.update(|cx| clear(cx)).ok();
            this.update(cx, |this, cx| {
                this.clearing = None;
                this.notices.clear();
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    pub fn dismiss(&mut self, id: u64, cx: &mut Context<Self>) {
        self.leaving.insert(id, Instant::now());
        let dismiss = self.props.on_dismiss.clone();
        let wait = if anim_off() { Duration::ZERO } else { ROW_OUT };
        cx.spawn_in_window(id, dismiss, wait);
        cx.notify();
    }

    fn row_frame(&self, index: usize, id: u64, now: Instant) -> (f32, f32, f32) {
        let row_ms = ROW_OUT.as_secs_f32();
        let start = match (self.clearing, self.leaving.get(&id)) {
            (Some(at), _) => Some(at + STAGGER * index.min(8) as u32),
            (None, Some(at)) => Some(*at),
            _ => None,
        };
        let Some(start) = start else { return (0., 1., 1.) };
        let p = (now.saturating_duration_since(start).as_secs_f32() / row_ms).clamp(0., 1.);
        let slide = ease_in_cubic(interval(p, 0., 0.7));
        let collapse = if self.clearing.is_some() { 1. } else { 1. - ease_in_out(interval(p, 0.55, 1.)) };
        (slide * TOAST_WIDTH, 1. - slide, collapse)
    }
}

trait SpawnDismiss {
    fn spawn_in_window(&mut self, id: u64, f: Handler<u64>, wait: Duration);
}

impl SpawnDismiss for Context<'_, NoticeCenter> {
    fn spawn_in_window(&mut self, id: u64, f: Handler<u64>, wait: Duration) {
        let Some(window) = self.windows().first().copied() else { return };
        self.spawn(async move |this, cx| {
            cx.background_executor().timer(wait).await;
            cx.update_window(window, |_, w, cx| f(id, w, cx)).ok();
            this.update(cx, |this, cx| {
                this.leaving.remove(&id);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

impl Render for NoticeCenter {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme(cx);
        let now = Instant::now();
        let me = cx.entity().downgrade();
        let unread = self.notices.iter().any(|n| !n.read);
        let p = self.props.clone();
        let small = |id: &'static str, glyph, tip: &'static str, color, disabled: bool, f: Callback| {
            IconButton::new(id, glyph).size(22., 15., 4.).tooltip(tip).color(color).disabled(disabled).on_click(move |w, cx| f(w, cx))
        };
        let (dnd_glyph, dnd_tip, dnd_color) = if self.dnd {
            (icon::NOTIFICATIONS_OFF_ROUNDED, "Não perturbe ligado · os avisos não aparecem na tela", t.accent)
        } else {
            (icon::NOTIFICATIONS_OFF_OUTLINED, "Não perturbe · avisos só aqui, sem aparecer na tela", t.fg_dim)
        };
        let clear_me = me.clone();
        let header = div().h(px(36.)).flex().items_center().pl(px(14.)).pr(px(6.)).border_b_1().border_color(hsla(t.border))
            .child(div().flex_1().text_size(px(12.5)).font_weight(FontWeight::BOLD).text_color(hsla(t.fg)).child("notificações"))
            .child(small("dnd", dnd_glyph, dnd_tip, dnd_color, false, p.on_toggle_dnd.clone()))
            .child(small("read-all", icon::DONE_ALL_ROUNDED, "Marcar tudo como lido", t.fg_dim, !unread, p.on_read_all.clone()))
            .child(small("clear-all", icon::CLEAR_ALL_ROUNDED, "Limpar tudo", t.fg_dim, self.notices.is_empty(), Rc::new(move |_, cx| {
                clear_me.update(cx, |this, cx| this.clear_all(cx)).ok();
            })))
            .child(small("close", icon::EXPAND_MORE_ROUNDED, "Fechar", t.fg_dim, false, p.on_close.clone()));
        let dismiss_me = me.clone();
        let hover_me = me.clone();
        let handlers = Rc::new(RowHandlers {
            on_open: p.on_open.clone(),
            on_close: Rc::new(move |id, _, cx| {
                dismiss_me.update(cx, |this, cx| this.dismiss(id, cx)).ok();
            }),
            on_hover: Some(Rc::new(move |(id, on), _, cx| {
                hover_me.update(cx, |this, cx| {
                    if on {
                        this.hovered = Some(id);
                    } else if this.hovered == Some(id) {
                        this.hovered = None;
                    }
                    cx.notify();
                }).ok();
            })),
        });
        let mut animating = self.clearing.is_some() || !self.leaving.is_empty();
        let rows: Vec<AnyElement> = self.notices.iter().enumerate().map(|(i, n)| {
            let (dx, fade, collapse) = self.row_frame(i, n.id, now);
            let mut wrap = div().relative().left(px(dx)).opacity(fade).max_h(px(140. * collapse)).overflow_hidden();
            if i > 0 {
                wrap = wrap.border_t_1().border_color(hsla(t.border));
            }
            wrap.child(notice_row(n, false, self.hovered == Some(n.id), &handlers, &t, cx)).into_any_element()
        }).collect();
        let settle = self.clearing.map(|at| {
            let rows_end = STAGGER * self.notices.len().min(8) as u32 + ROW_OUT;
            (now.saturating_duration_since(at).saturating_sub(rows_end).as_secs_f32() / SETTLE.as_secs_f32()).clamp(0., 1.)
        });
        let body: AnyElement = match settle {
            _ if self.notices.is_empty() => empty_text(&t, 1.).into_any_element(),
            Some(s) if s > 0. => {
                let k = crate::chrome::anim::ease::ease_in_out_cubic(s);
                div().child(div().max_h(px(140. * self.notices.len() as f32 * (1. - k))).overflow_hidden().children(rows))
                    .child(div().max_h(px(80. * k)).overflow_hidden().child(empty_text(&t, interval(s, 0.4, 1.))))
                    .into_any_element()
            }
            _ => div().id("notice-list").max_h(px(420. - 36.)).overflow_y_scroll().children(rows).into_any_element(),
        };
        let enter = (now.duration_since(self.born).as_secs_f32() / 0.16).min(1.);
        animating |= enter < 1.;
        if animating && !anim_off() {
            window.request_animation_frame();
        }
        let e = if anim_off() { 1. } else { crate::chrome::anim::ease::ease_out(enter) };
        div()
            .id("notice-center")
            .debug_selector(|| "notice-center".into())
            .relative()
            .top(px(8. * (1. - e)))
            .opacity(e)
            .w(px(380.))
            .max_h(px(420.))
            .bg(hsla(t.bg_sidebar))
            .rounded(px(8.))
            .border_1()
            .border_color(alpha(t.accent, 0.7))
            .shadow(shadow(&t, 18., 6.))
            .overflow_hidden()
            .on_mouse_down_out({
                let close = p.on_close.clone();
                move |_, w, cx| close(w, cx)
            })
            .child(header)
            .child(body)
    }
}

fn empty_text(t: &mx_theme::Theme, opacity: f32) -> gpui::Div {
    div()
        .debug_selector(|| "notice-empty".into())
        .pl(px(14.)).pt(px(6.)).pr(px(14.)).pb(px(18.))
        .text_size(px(12.)).line_height(px(16.8)).text_color(hsla(t.fg_faint))
        .opacity(opacity)
        .child(EMPTY)
}
