use gpui::{AnyElement, IntoElement, ParentElement, Styled, div, px};
use mx_core::model::{ClaudeStatus, TabKind};

use crate::app::avatar::claude_avatar;
use crate::app::icons::{Icon, icon, terminal_mark};
use crate::app::theme::Mx;

pub fn tab_glyph(mx: &Mx, kind: TabKind, status: ClaudeStatus, done: bool, asleep: bool, box_width: f32) -> AnyElement {
    let slot = div().flex_none().w(px(box_width)).flex().items_center();
    match kind {
        TabKind::Claude => slot.child(claude_avatar(mx, status, 26., done, asleep)).into_any_element(),
        TabKind::Reader => slot.child(icon(Icon::Article, 18., mx.fg_dim)).into_any_element(),
        TabKind::Setup => slot.child(icon(Icon::Tune, 18., mx.fg_dim)).into_any_element(),
        TabKind::Plugin => slot.child(icon(Icon::Terminal, 18., mx.fg_dim)).into_any_element(),
        TabKind::Shell => slot.child(terminal_mark(22., mx.fg_dim)).into_any_element(),
    }
}
