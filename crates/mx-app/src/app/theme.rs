use std::sync::Arc;

use gpui::{App, BoxShadow, Global, Hsla, Rgba, point, px};
use mx_core::model::ClaudeStatus;
use mx_store::TabTint;
use mx_theme::{Color, Theme};

pub const GAP: f32 = 8.;
pub const RADIUS: f32 = 10.;

pub fn hsla(c: Color) -> Hsla {
    Rgba {
        r: c.red() as f32,
        g: c.green() as f32,
        b: c.blue() as f32,
        a: c.alpha() as f32,
    }
    .into()
}

#[derive(Clone)]
pub struct Mx {
    pub theme: Arc<Theme>,
    pub canvas: Hsla,
    pub bg: Hsla,
    pub bg_sidebar: Hsla,
    pub bg_hover: Hsla,
    pub bg_active: Hsla,
    pub border: Hsla,
    pub fg: Hsla,
    pub fg_dim: Hsla,
    pub fg_faint: Hsla,
    pub accent: Hsla,
    pub green: Hsla,
    pub yellow: Hsla,
    pub red: Hsla,
    pub purple: Hsla,
    pub claude: Hsla,
    pub shadow: Hsla,
    group_tints: [Hsla; 4],
}

impl Global for Mx {}

impl Mx {
    pub fn new(theme: Arc<Theme>) -> Mx {
        let p = theme.palette();
        let tints = p.group_tints();
        Mx {
            canvas: hsla(p.canvas),
            bg: hsla(p.bg),
            bg_sidebar: hsla(p.bg_sidebar),
            bg_hover: hsla(p.bg_hover),
            bg_active: hsla(p.bg_active),
            border: hsla(p.border),
            fg: hsla(p.fg),
            fg_dim: hsla(p.fg_dim),
            fg_faint: hsla(p.fg_faint),
            accent: hsla(p.accent),
            green: hsla(p.green),
            yellow: hsla(p.yellow),
            red: hsla(p.red),
            purple: hsla(p.purple),
            claude: hsla(mx_theme::CLAUDE),
            shadow: hsla(p.shadow()),
            group_tints: tints.map(hsla),
            theme,
        }
    }

    pub fn get(cx: &App) -> &Mx {
        cx.global::<Mx>()
    }

    pub fn install(theme_id: &str, cx: &mut App) {
        let theme = mx_theme::built_in(theme_id).cloned().unwrap_or_else(mx_theme::default_theme);
        crate::chrome::style::set_theme(theme.clone(), cx);
        cx.set_global(Mx::new(theme.clone()));
    }

    pub fn status(&self, s: ClaudeStatus) -> Hsla {
        match s {
            ClaudeStatus::Working | ClaudeStatus::Tool => self.accent,
            ClaudeStatus::WaitingInput | ClaudeStatus::WaitingAnswer => self.yellow,
            ClaudeStatus::WaitingPermission => self.red,
            ClaudeStatus::Idle | ClaudeStatus::Ready => self.green,
            ClaudeStatus::Starting => self.purple,
            ClaudeStatus::Unknown | ClaudeStatus::Ended => self.fg_faint,
        }
    }

    pub fn tint(&self, t: mx_core::model::Tint) -> Hsla {
        mx_theme::Tint::by_name(Some(t.name()))
            .map_or(self.accent, |t| hsla(self.theme.tint(t)))
    }

    pub fn tab_tint(&self, t: TabTint) -> Hsla {
        match t {
            TabTint::Chosen(t) => self.tint(t),
            TabTint::Group(i) => self.group_tints[i % self.group_tints.len()],
        }
    }

    pub fn card_shadow(&self, blur: f32, dy: f32) -> Vec<BoxShadow> {
        vec![BoxShadow {
            color: self.shadow,
            offset: point(px(0.), px(dy)),
            blur_radius: px(blur),
            spread_radius: px(0.),
        }]
    }
}

pub fn blend(top: Hsla, alpha: f32, under: Hsla) -> Hsla {
    let a = Rgba::from(top);
    let b = Rgba::from(under);
    let mix = |x: f32, y: f32| x * alpha + y * (1. - alpha);
    Rgba {
        r: mix(a.r, b.r),
        g: mix(a.g, b.g),
        b: mix(a.b, b.b),
        a: 1.,
    }
    .into()
}

pub fn on_color(bg: Hsla, canvas: Hsla) -> Hsla {
    if bg.l > 0.6 { canvas } else { gpui::white() }
}
