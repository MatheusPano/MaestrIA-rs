use std::rc::Rc;
use std::sync::Arc;

use gpui::{App, BoxShadow, Context, Global, Hsla, Rgba, WeakEntity, Window, point, px};
use mx_theme::{Color, Theme};

pub type Callback = Rc<dyn Fn(&mut Window, &mut App)>;
pub type Handler<T> = Rc<dyn Fn(T, &mut Window, &mut App)>;

pub const MONO: &str = "Hack";

pub struct CurrentTheme(pub Arc<Theme>);

impl Global for CurrentTheme {}

pub fn theme(cx: &App) -> Arc<Theme> {
    cx.try_global::<CurrentTheme>()
        .map(|t| t.0.clone())
        .unwrap_or_else(|| mx_theme::default_theme().clone())
}

pub fn set_theme(theme: Arc<Theme>, cx: &mut App) {
    cx.set_global(CurrentTheme(theme));
}

pub fn hsla(color: Color) -> Hsla {
    Rgba {
        r: color.red() as f32,
        g: color.green() as f32,
        b: color.blue() as f32,
        a: color.alpha() as f32,
    }
    .into()
}

pub fn alpha(color: Color, a: f64) -> Hsla {
    hsla(color.with_alpha(a))
}

pub fn on(bg: Color, canvas: Color) -> Hsla {
    if bg.luminance() > 0.5 {
        hsla(canvas)
    } else {
        hsla(Color::WHITE)
    }
}

pub fn shadow(theme: &Theme, blur: f32, dy: f32) -> Vec<BoxShadow> {
    vec![BoxShadow {
        color: hsla(theme.shadow()),
        offset: point(px(0.), px(dy)),
        blur_radius: px(blur),
        spread_radius: px(0.),
    }]
}

pub fn cb<V: 'static>(
    this: WeakEntity<V>,
    f: impl Fn(&mut V, &mut Window, &mut Context<V>) + 'static,
) -> impl Fn(&mut Window, &mut App) + 'static {
    move |window, cx| {
        this.update(cx, |v, cx| f(v, window, cx)).ok();
    }
}
