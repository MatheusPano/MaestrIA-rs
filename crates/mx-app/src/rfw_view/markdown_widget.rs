use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use gpui::{ParentElement, Styled, px};
use mx_claude::markdown::{MdNode, parse_markdown};
use mx_rfw::widgets::maestria::Markdown;

use super::layout::{Ctx, El};
use super::selectable::{notifier, selection_color};
use super::zoom_view::scaled;
use crate::reader::select::{SelectionSlot, area};
use crate::reader::{FollowHref, Painter, SANS, Sheet};

const TABLE_WIDTH: f32 = 560.;
const MAX_PARSED: usize = 512;

thread_local! {
    static PARSED: RefCell<HashMap<String, Rc<MdNode>>> = RefCell::new(HashMap::new());
}

fn parsed(text: &str) -> Rc<MdNode> {
    PARSED.with(|p| {
        let mut p = p.borrow_mut();
        if p.len() > MAX_PARSED {
            p.clear();
        }
        p.entry(text.to_owned()).or_insert_with(|| Rc::new(parse_markdown(text))).clone()
    })
}

pub fn render(cx: &mut Ctx, m: &Markdown) -> El {
    let el = cx.el().font_family(SANS);
    let Some(states) = cx.widgets else { return el.child(m.text.clone()) };
    let Some(text_system) = states.text_system.borrow().clone() else { return el.child(m.text.clone()) };
    let size = scaled(m.font_size as f32);
    let sheet = Sheet::new(cx.palette, size - crate::reader::BASE_SIZE);
    let follow: Option<FollowHref> = states.follow.clone().map(|f| -> FollowHref { Rc::new(move |href: &str, window, cx| f(href.to_owned(), window, cx)) });
    let outer = states.selection();
    let own = outer.is_none();
    let slot: SelectionSlot = outer.unwrap_or_else(|| states.selections.next());
    if own {
        let mut st = slot.borrow_mut();
        st.begin_frame();
        st.color = Some(selection_color(cx.palette));
    }
    let tree = parsed(&m.text);
    let body = Painter::new(&sheet, follow, text_system, px(scaled(TABLE_WIDTH))).with_selection(Some(slot.clone())).document(&tree);
    let el = el.text_color(sheet.fg).child(body);
    if !own {
        return el;
    }
    slot.borrow_mut().end_frame();
    if cx.interactive { area(el, &slot, notifier(cx)) } else { el }
}
