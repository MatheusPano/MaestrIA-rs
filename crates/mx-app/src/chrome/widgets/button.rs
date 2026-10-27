use gpui::{
    App, ElementId, FontWeight, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    SharedString, StatefulInteractiveElement, Styled, Window, div, px,
};
use mx_theme::Color;

use crate::chrome::icon::{Icon, icon};
use crate::chrome::style::{Callback, alpha, hsla, on, theme};
use crate::chrome::widgets::tip;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonKind {
    Text,
    Primary,
    Danger,
    Success,
}

#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    label: SharedString,
    kind: ButtonKind,
    leading: Option<Icon>,
    disabled: bool,
    on_click: Option<Callback>,
}

impl Button {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>, kind: ButtonKind) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            kind,
            leading: None,
            disabled: false,
            on_click: None,
        }
    }

    pub fn leading(mut self, icon: Icon) -> Self {
        self.leading = Some(icon);
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn on_click(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(std::rc::Rc::new(f));
        self
    }

    pub fn callback(mut self, f: Option<Callback>) -> Self {
        self.on_click = f;
        self
    }
}

impl RenderOnce for Button {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme(cx);
        let fill: Option<Color> = match self.kind {
            ButtonKind::Text => None,
            ButtonKind::Primary => Some(t.accent),
            ButtonKind::Danger => Some(t.red),
            ButtonKind::Success => Some(t.green),
        };
        let fg = match fill {
            Some(c) => on(c, t.canvas),
            None => hsla(t.accent),
        };
        let label = self.label.clone();
        let mut el = div()
            .id(self.id)
            .debug_selector(|| format!("button:{label}"))
            .flex()
            .flex_none()
            .items_center()
            .gap(px(6.))
            .h(px(34.))
            .px(px(14.))
            .rounded(px(18.))
            .text_size(px(13.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(fg);
        if let Some(c) = fill {
            el = el.bg(hsla(c));
        }
        if let Some(i) = self.leading {
            el = el.child(icon(i, 16., fg));
        }
        el = el.child(self.label);
        if self.disabled {
            return el.opacity(0.38);
        }
        let hover = match fill {
            Some(c) => hsla(c.with_alpha(0.88)),
            None => alpha(t.accent, 0.1),
        };
        el = el.cursor_pointer().hover(move |s| s.bg(hover));
        if let Some(f) = self.on_click {
            el = el.on_click(move |_, window, cx| f(window, cx));
        }
        el
    }
}

#[derive(IntoElement)]
pub struct IconButton {
    id: ElementId,
    icon: Icon,
    size: f32,
    icon_size: f32,
    radius: f32,
    color: Option<Color>,
    hover_bg: Option<Color>,
    tooltip: Option<SharedString>,
    selector: Option<SharedString>,
    disabled: bool,
    on_click: Option<Callback>,
}

impl IconButton {
    pub fn new(id: impl Into<ElementId>, icon: Icon) -> Self {
        Self {
            id: id.into(),
            icon,
            size: 22.,
            icon_size: 15.,
            radius: 4.,
            color: None,
            hover_bg: None,
            tooltip: None,
            selector: None,
            disabled: false,
            on_click: None,
        }
    }

    pub fn selector(mut self, selector: impl Into<SharedString>) -> Self {
        self.selector = Some(selector.into());
        self
    }

    pub fn size(mut self, size: f32, icon_size: f32, radius: f32) -> Self {
        self.size = size;
        self.icon_size = icon_size;
        self.radius = radius;
        self
    }

    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    pub fn hover_bg(mut self, color: Color) -> Self {
        self.hover_bg = Some(color);
        self
    }

    pub fn tooltip(mut self, text: impl Into<SharedString>) -> Self {
        self.tooltip = Some(text.into());
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn on_click(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(std::rc::Rc::new(f));
        self
    }
}

impl RenderOnce for IconButton {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme(cx);
        let color = if self.disabled {
            alpha(t.fg_faint, 0.5)
        } else {
            hsla(self.color.unwrap_or(t.fg_dim))
        };
        let selector = self.selector.clone().unwrap_or_else(|| format!("icon:{}", self.tooltip.clone().unwrap_or_default()).into());
        let mut el = div()
            .id(self.id)
            .debug_selector(move || selector.to_string())
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(px(self.size))
            .rounded(px(self.radius))
            .child(icon(self.icon, self.icon_size, color));
        if let Some(text) = self.tooltip {
            el = el.tooltip(tip(text));
        }
        if self.disabled {
            return el;
        }
        let hover = hsla(self.hover_bg.unwrap_or(t.bg_hover));
        el = el.cursor_pointer().hover(move |s| s.bg(hover));
        if let Some(f) = self.on_click {
            el = el.on_click(move |_, window, cx| {
                cx.stop_propagation();
                f(window, cx)
            });
        }
        el
    }
}
