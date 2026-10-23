use std::path::PathBuf;
use std::rc::Rc;

use gpui::{
    AnyElement, App, AppContext, Entity, FontWeight, InteractiveElement, IntoElement, ParentElement,
    SharedString, StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};
use mx_store::Store;

use crate::app::drag::{DraggedTab, TabGhost};
use crate::app::host::{StoreHost, update_store};
use crate::app::icons::{Icon, icon};
use crate::app::theme::Mx;
use crate::app::tooltip::tip;
use crate::panes::header::HEIGHT;
use crate::rfw_view::{named_icon, plugin_glyph};

pub type Opener = Rc<dyn Fn(String, &mut Window, &mut App)>;

pub const RELEASE_TIP: &str = "levar pra tela das sessões, ao lado do painel em foco lá";

pub fn return_tip(name: &str) -> String {
    format!("devolver pra tela do {name}")
}

pub struct ViewHeader {
    id: String,
    title: String,
    plugin: String,
    plugin_name: String,
    icon: Option<String>,
    dir: Option<PathBuf>,
    slot: Option<String>,
    settings: bool,
    screen: Option<bool>,
    pinned: Option<bool>,
    dim: bool,
}

impl ViewHeader {
    pub fn of(store: &Store, id: &str, show_focus: bool, focused: bool) -> Option<ViewHeader> {
        let t = store.tab(id)?;
        let v = store.plugin_view(id)?;
        let info = store.plugins().and_then(|r| r.get(&v.plugin_id));
        let manifest = info.as_ref().and_then(|p| p.manifest.clone());
        let screen = store.home_screen_of(id).map(|_| !t.on_sessions);
        Some(ViewHeader {
            id: id.to_owned(),
            title: store.title(t),
            plugin: v.plugin_id.clone(),
            plugin_name: v.plugin_name.clone(),
            icon: manifest.as_ref().and_then(|m| m.icon.clone()),
            dir: info.map(|p| p.dir),
            slot: store.tabs().iter().position(|x| x.id == id).and_then(crate::app::keys::slot),
            settings: manifest.is_some_and(|m| !m.settings.is_empty()),
            screen,
            pinned: (t.pinned || store.pane_count() > 1).then_some(t.pinned),
            dim: show_focus && !focused,
        })
    }

    fn button(&self, mx: &Mx, name: &str, glyph: AnyElement, tooltip: SharedString, act: impl Fn(&mut Window, &mut App) + 'static) -> AnyElement {
        div()
            .id(SharedString::from(format!("view-{name}:{}", self.id)))
            .flex_none()
            .size(px(26.))
            .rounded(px(6.))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .hover(|s| s.bg(mx.bg_hover))
            .tooltip(tip(tooltip))
            .on_click(move |_, window, cx: &mut App| {
                cx.stop_propagation();
                act(window, cx)
            })
            .child(glyph)
            .into_any_element()
    }

    fn store_button(&self, mx: &Mx, host: &Entity<StoreHost>, name: &str, glyph: AnyElement, tooltip: impl Into<SharedString>, act: impl Fn(&mut Store, &str) + 'static) -> AnyElement {
        let host = host.clone();
        let id = self.id.clone();
        self.button(mx, name, glyph, tooltip.into(), move |_, cx| update_store(&host, cx, |s| act(s, &id)))
    }

    pub fn render(self, mx: &Mx, host: &Entity<StoreHost>, settings: Option<Opener>, width: f32) -> impl IntoElement {
        let faint = mx.fg_faint;
        let mut buttons: Vec<AnyElement> = Vec::new();
        if let (true, Some(open)) = (self.settings, settings) {
            let plugin = self.plugin.clone();
            let glyph = named_icon("settings_outlined", 16., faint);
            let tooltip = SharedString::from(format!("Configurar {}", self.plugin_name));
            buttons.push(self.button(mx, "settings", glyph, tooltip, move |w, cx| open(plugin.clone(), w, cx)));
        }
        match self.screen {
            Some(true) => buttons.push(self.store_button(mx, host, "release", named_icon("open_in_new_rounded", 16., faint), RELEASE_TIP, |s, id| s.release_view(id))),
            Some(false) => buttons.push(self.store_button(mx, host, "return", named_icon("undo_rounded", 16., faint), return_tip(&self.plugin_name), |s, id| s.return_view(id))),
            None => {}
        }
        if let Some(pinned) = self.pinned {
            let (glyph, color) = if pinned { (Icon::PushPin, mx.accent) } else { (Icon::PushPinOutlined, faint) };
            let tooltip = if pinned { "Solto" } else { "Prender" };
            buttons.push(self.store_button(mx, host, "pin", icon(glyph, 16., color).into_any_element(), tooltip, |s, id| s.toggle_pin(id)));
        }
        buttons.push(self.store_button(mx, host, "close", icon(Icon::CloseRounded, 16., faint).into_any_element(), "Fechar esta janela", |s, id| {
            s.close_tab(id);
        }));
        let lead = plugin_glyph(self.icon.as_deref(), self.dir.as_deref(), 17., mx.fg_dim);
        let fade = if self.dim { 0.55 } else { 1. };
        let drag = DraggedTab { id: self.id.clone(), title: self.title.clone(), folder_root: String::new() };
        let subtitle = (width >= 190.).then(|| format!("Plugin · {}", self.plugin_name));
        div()
            .id(SharedString::from(format!("view-header:{}", self.id)))
            .h(px(HEIGHT))
            .flex_none()
            .pl(px(12.))
            .pr(px(8.))
            .flex()
            .items_center()
            .bg(mx.bg_sidebar)
            .border_b_1()
            .border_color(mx.border)
            .on_drag(drag, |d: &DraggedTab, at, _, cx| cx.new(|_| TabGhost { title: d.title.clone(), at }))
            .child(lead)
            .child(div().w(px(9.)))
            .child(title_line(mx, &self.title))
            .when_some(self.slot, |d, slot| {
                d.child(div().flex_none().ml(px(6.)).font_family("Hack").text_size(px(10.5)).text_color(mx.fg_faint).child(slot))
            })
            .child(
                div()
                    .ml(px(10.))
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .opacity(fade)
                    .text_size(px(12.))
                    .text_color(mx.fg_dim)
                    .children(subtitle),
            )
            .child(div().flex_none().flex().items_center().gap(px(2.)).opacity(fade).children(buttons))
    }
}

fn title_line(mx: &Mx, title: &str) -> impl IntoElement {
    div()
        .flex_none()
        .max_w(px(320.))
        .truncate()
        .text_size(px(15.))
        .font_weight(FontWeight::BOLD)
        .text_color(mx.fg)
        .child(title.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_screen_buttons_say_where_the_window_goes() {
        assert_eq!(RELEASE_TIP, "levar pra tela das sessões, ao lado do painel em foco lá");
        assert_eq!(return_tip("Wiboor"), "devolver pra tela do Wiboor");
    }
}
