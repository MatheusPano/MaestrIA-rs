use gpui::{
    AnyElement, App, AppContext, Context, Entity, FontWeight, Hsla, InteractiveElement, IntoElement, MouseButton,
    ParentElement, Render, SharedString, StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};
use mx_core::model::Tab;
use mx_store::Store;

use super::cards_model::{self as model, Bucket, GAP, NOTHING_SAID, StateTone};
use crate::app::avatar::dot;
use crate::app::drag::DraggedTab;
use crate::app::host::{StoreHost, update_store};
use crate::app::icons::{Icon, icon};
use crate::app::panel_actions::open_panel_menu;
use crate::app::theme::{Mx, RADIUS};
use crate::sidebar::glyph::tab_glyph;

const HEIGHT: f32 = 118.;

pub fn section_header(mx: &Mx, label: &str, color: Option<Hsla>, count: usize) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .when_some(color, |d, c| d.child(dot(c, 6.)).child(div().w(px(8.))))
        .child(div().text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).text_color(mx.fg_dim).child(label.to_owned()))
        .child(div().w(px(6.)))
        .child(div().font_family("Hack").text_size(px(11.5)).text_color(mx.fg_faint).child(count.to_string()))
}

fn bucket_color(mx: &Mx, b: Bucket) -> Hsla {
    match b {
        Bucket::Waiting => mx.yellow,
        Bucket::Fresh => mx.green,
        Bucket::Working => mx.accent,
        Bucket::Rest => mx.fg_faint,
    }
}

pub fn session_cards(mx: &Mx, host: &Entity<StoreHost>, store: &Store, width: f32) -> impl IntoElement {
    let tabs = model::card_tabs(store);
    let (_, card_width) = model::columns(width, GAP, model::CARD_MIN);
    let home = std::env::var("HOME").ok();
    let mut col = div().flex().flex_col();
    for (bucket, list) in model::sections(&tabs) {
        let cards: Vec<AnyElement> =
            list.iter().map(|t| Card::of(store, mx, t, home.as_deref()).render(mx, host, card_width)).collect();
        col = col
            .child(section_header(mx, bucket.label(), Some(bucket_color(mx, bucket)), list.len()))
            .child(div().h(px(8.)))
            .child(div().flex().flex_wrap().gap(px(GAP)).children(cards))
            .child(div().h(px(22.)));
    }
    col
}

#[derive(Clone)]
struct Card {
    id: String,
    title: String,
    folder_root: String,
    where_: String,
    body: Option<(String, bool)>,
    state: (&'static str, Hsla),
    border: Hsla,
    tint: Option<Hsla>,
    hotkey: Option<String>,
    done: bool,
    follow_ups: usize,
    ago: Option<(String, bool)>,
    glyph: (mx_core::model::TabKind, mx_core::model::ClaudeStatus),
    asleep: bool,
}

impl Card {
    fn of(store: &Store, mx: &Mx, t: &Tab, home: Option<&str>) -> Card {
        let title = store.title(t);
        let folder = store.folder_of(t);
        let (label, tone) = model::state(t);
        let color = match tone {
            StateTone::Status(s) => mx.status(s),
            StateTone::Faint => mx.fg_faint,
        };
        Card {
            id: t.id.clone(),
            where_: model::where_line(&title, &folder.name, folder.is_loose, &t.branch),
            title,
            folder_root: t.folder_root.clone(),
            body: model::body(t, &store.subtitle(t), home),
            state: (label, color),
            border: if model::attention(t) { mx.status(t.status()).opacity(0.45) } else { mx.border },
            tint: store.tint_of(&t.id).map(|tt| mx.tab_tint(tt)),
            hotkey: t.hotkey.as_deref().map(mx_store::hotkey_label),
            done: t.done,
            follow_ups: t.follow_ups.len(),
            ago: t.rested_ago(&chrono::Local::now()).map(|a| (a, t.unseen)),
            glyph: (t.kind, t.status()),
            asleep: t.hibernated,
        }
    }

    fn face(&self, mx: &Mx, width: f32) -> gpui::Div {
        let head = div()
            .flex()
            .items_center()
            .child(div().w(px(30.)).flex().justify_center().child(tab_glyph(mx, self.glyph.0, self.glyph.1, self.done, self.asleep, 30.)))
            .child(div().w(px(10.)))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .truncate()
                            .text_size(px(13.5))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(if self.done { mx.fg_dim } else { mx.fg })
                            .child(self.title.clone()),
                    )
                    .when(!self.where_.is_empty(), |d| {
                        d.child(div().truncate().font_family("Hack").text_size(px(10.5)).text_color(mx.fg_faint).child(self.where_.clone()))
                    }),
            )
            .when_some(self.hotkey.clone(), |d, k| {
                d.child(
                    div().px(px(5.)).py(px(1.5)).rounded(px(4.)).bg(mx.bg_active).font_family("Hack").text_size(px(10.)).text_color(mx.fg_dim).child(k),
                )
            });
        let body = match &self.body {
            Some((text, path)) => div()
                .line_clamp(2)
                .text_ellipsis()
                .overflow_hidden()
                .text_color(mx.fg_dim)
                .when(*path, |d| d.font_family("Hack").text_size(px(11.5)).line_height(px(16.1)))
                .when(!*path, |d| d.text_size(px(12.)).line_height(px(16.8)))
                .child(text.clone()),
            None => div().text_size(px(12.)).italic().text_color(mx.fg_faint).child(NOTHING_SAID),
        };
        let (label, color) = self.state;
        let mut foot = div()
            .flex()
            .items_center()
            .child(dot(color, 6.))
            .child(div().w(px(6.)))
            .child(div().text_size(px(11.)).text_color(color).child(label))
            .child(div().flex_1());
        if self.done {
            foot = foot.child(div().ml(px(6.)).child(icon(Icon::TaskAlt, 13., mx.green)));
        }
        if self.follow_ups > 0 {
            foot = foot.child(div().ml(px(6.)).child(icon(Icon::Replay, 13., mx.accent)));
        }
        if let Some((ago, unseen)) = &self.ago {
            foot = foot.child(div().ml(px(8.)).text_size(px(11.)).text_color(if *unseen { mx.green } else { mx.fg_faint }).child(ago.clone()));
        }
        div()
            .relative()
            .w(px(width))
            .h(px(HEIGHT))
            .rounded(px(RADIUS))
            .overflow_hidden()
            .bg(mx.bg_sidebar)
            .border_1()
            .border_color(self.border)
            .when_some(self.tint, |d, tint| d.child(div().absolute().left_0().top_0().bottom_0().w(px(3.)).bg(tint)))
            .child(
                div()
                    .size_full()
                    .pl(px(14.))
                    .pt(px(12.))
                    .pr(px(12.))
                    .pb(px(10.))
                    .flex()
                    .flex_col()
                    .child(head)
                    .child(div().h(px(9.)))
                    .child(div().flex_1().min_h_0().child(body))
                    .child(foot),
            )
    }

    fn render(self, mx: &Mx, host: &Entity<StoreHost>, width: f32) -> AnyElement {
        let (accent, hover, shadow) = (mx.accent, mx.bg_hover, mx.card_shadow(16., 6.));
        let (click_host, menu_id, click_id) = (host.clone(), self.id.clone(), self.id.clone());
        let drag = DraggedTab { id: self.id.clone(), title: self.title.clone(), folder_root: self.folder_root.clone() };
        let ghost = CardGhost { card: self.clone(), width };
        self.face(mx, width)
            .id(SharedString::from(format!("card:{}", self.id)))
            .cursor_pointer()
            .hover(move |s| s.bg(hover).border_color(accent.opacity(0.6)).shadow(shadow.clone()))
            .on_click(move |_, _, cx: &mut App| update_store(&click_host, cx, |s| model::open_card(s, &click_id)))
            .on_mouse_down(MouseButton::Right, move |ev, _, cx| {
                cx.stop_propagation();
                open_panel_menu(&menu_id, ev.position, cx)
            })
            .on_drag(drag, move |_, _, _, cx| cx.new(|_| ghost.clone()))
            .into_any_element()
    }
}

#[derive(Clone)]
struct CardGhost {
    card: Card,
    width: f32,
}

impl Render for CardGhost {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mx = Mx::get(cx).clone();
        self.card.face(&mx, self.width).opacity(0.92)
    }
}
