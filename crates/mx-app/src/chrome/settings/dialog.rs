use std::collections::HashSet;
use std::rc::Rc;

use gpui::{
    AnyElement, App, AppContext, Context, Entity, FocusHandle, FontWeight, InteractiveElement,
    IntoElement, KeyDownEvent, ParentElement, Pixels, Point, Render, ScrollHandle, SharedString,
    StatefulInteractiveElement, Styled, Window, div, px,
};

use super::model::{ChordCheck, KeyPress, PluginMenuAction, Section, SettingsAction, SettingsData};
use crate::chrome::dialog::frame::{barrier, card};
use crate::chrome::icon::{self, Icon, icon};
use crate::chrome::menu::{ContextMenu, MenuEntry};
use crate::chrome::style::{Handler, cb, hsla, theme};
use crate::chrome::widgets::{Button, ButtonKind, wide_tip};

pub type Validate = Rc<dyn Fn(&KeyPress) -> ChordCheck>;

pub struct Recording {
    pub action: String,
    pub replacing: Option<String>,
    pub rejection: Option<String>,
}

pub struct SettingsDialog {
    pub(super) section: Section,
    pub(super) data: SettingsData,
    pub(super) on_action: Handler<SettingsAction>,
    pub(super) validate: Validate,
    pub(super) recording: Option<Recording>,
    pub(super) expanded: HashSet<String>,
    pub(super) menu: Option<Entity<ContextMenu<PluginMenuAction>>>,
    pub(super) focus: FocusHandle,
    pub(crate) scroll: ScrollHandle,
}

fn section_icon(s: Section) -> Icon {
    match s {
        Section::Appearance => icon::PALETTE_OUTLINED,
        Section::Shortcuts => icon::KEYBOARD_OUTLINED,
        Section::Programs => icon::ROCKET_LAUNCH_OUTLINED,
        Section::Plugins => icon::EXTENSION_OUTLINED,
        Section::Sessions => icon::BEDTIME_OUTLINED,
        Section::Account => icon::SPEED_OUTLINED,
    }
}

impl SettingsDialog {
    pub fn new(
        data: SettingsData,
        section: Section,
        validate: Validate,
        on_action: impl Fn(SettingsAction, &mut Window, &mut App) + 'static,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            section,
            data,
            on_action: Rc::new(on_action),
            validate,
            recording: None,
            expanded: HashSet::new(),
            menu: None,
            focus: cx.focus_handle(),
            scroll: ScrollHandle::new(),
        }
    }

    pub fn set_data(&mut self, data: SettingsData, cx: &mut Context<Self>) {
        self.data = data;
        cx.notify();
    }

    #[cfg(test)]
    pub fn section(&self) -> Section {
        self.section
    }

    pub fn show(&mut self, section: Section, cx: &mut Context<Self>) {
        self.section = section;
        self.recording = None;
        self.scroll.set_offset(gpui::point(px(0.), px(0.)));
        cx.notify();
    }

    pub fn act(&self, action: SettingsAction, window: &mut Window, cx: &mut App) {
        (self.on_action)(action, window, cx);
    }

    pub(super) fn send(&self, action: SettingsAction) -> impl Fn(&mut Window, &mut App) + 'static {
        let f = self.on_action.clone();
        move |w, cx| f(action.clone(), w, cx)
    }

    pub fn record(&mut self, action: String, replacing: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        self.recording = Some(Recording { action, replacing, rejection: None });
        window.focus(&self.focus);
        cx.notify();
    }

    pub fn preset_recording(&mut self, action: &str, rejection: String) {
        self.recording = Some(Recording {
            action: action.to_owned(),
            replacing: None,
            rejection: (!rejection.is_empty()).then_some(rejection),
        });
    }

    pub fn recording(&self) -> Option<&Recording> {
        self.recording.as_ref()
    }

    pub fn key(&mut self, press: KeyPress, window: &mut Window, cx: &mut Context<Self>) {
        let Some(rec) = self.recording.as_mut() else { return };
        if press.key == "escape" && !(press.ctrl || press.alt || press.meta || press.shift) {
            self.recording = None;
            cx.notify();
            return;
        }
        match (self.validate)(&press) {
            ChordCheck::Ignore => {}
            ChordCheck::Rejected(why) => {
                rec.rejection = Some(why);
                cx.notify();
            }
            ChordCheck::Accepted { id, .. } => {
                let rec = self.recording.take().expect("recording");
                (self.on_action)(SettingsAction::Bind { action: rec.action, chord: id, replacing: rec.replacing }, window, cx);
                cx.notify();
            }
        }
    }

    fn key_down(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.recording.is_none() {
            return;
        }
        let k = &ev.keystroke;
        let press = KeyPress {
            key: k.key.clone(),
            ctrl: k.modifiers.control,
            alt: k.modifiers.alt,
            shift: k.modifiers.shift,
            meta: k.modifiers.platform,
        };
        self.key(press, window, cx);
        cx.stop_propagation();
    }

    pub(super) fn open_menu(&mut self, entries: Vec<MenuEntry<PluginMenuAction>>, at: Point<Pixels>, plugin: String, cx: &mut Context<Self>) {
        let on_action = self.on_action.clone();
        let me = cx.entity().downgrade();
        let me2 = me.clone();
        self.menu = Some(cx.new(|_| {
            ContextMenu::new(
                entries,
                at,
                move |a, w, cx| on_action(SettingsAction::Plugin(plugin.clone(), a), w, cx),
                move |_, cx| {
                    me2.update(cx, |this, cx| {
                        this.menu = None;
                        cx.notify();
                    })
                    .ok();
                },
            )
        }));
        let _ = me;
        cx.notify();
    }

    fn rail(&self, cx: &mut Context<Self>) -> AnyElement {
        let t = theme(cx);
        let items = Section::ALL.iter().map(|s| {
            let s = *s;
            let on = s == self.section;
            div()
                .id(SharedString::from(format!("section-{}", s.label())))
                .debug_selector(move || format!("section:{}", s.label()))
                .flex()
                .items_center()
                .gap(px(9.))
                .mx(px(8.))
                .my(px(1.))
                .px(px(10.))
                .py(px(8.))
                .rounded(px(7.))
                .cursor_pointer()
                .when_bg(on, hsla(t.bg_active))
                .hover(|st| st.bg(hsla(t.bg_hover)))
                .child(icon(section_icon(s), 15., hsla(if on { t.accent } else { t.fg_dim })))
                .child(
                    div()
                        .text_size(px(12.5))
                        .when_weight(on)
                        .text_color(hsla(if on { t.fg } else { t.fg_dim }))
                        .child(s.label()),
                )
                .on_click(cx.listener(move |this, _, _, cx| this.show(s, cx)))
        });
        div()
            .w(px(184.))
            .flex_none()
            .h_full()
            .bg(hsla(t.bg))
            .border_r_1()
            .border_color(hsla(t.border))
            .child(
                div()
                    .pl(px(16.))
                    .pt(px(18.))
                    .pr(px(16.))
                    .pb(px(14.))
                    .text_size(px(13.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(hsla(t.fg))
                    .child("Configurações"),
            )
            .children(items)
            .into_any_element()
    }

    fn body(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        match self.section {
            Section::Appearance => super::appearance::page(self, window, cx),
            Section::Shortcuts => super::shortcuts::page(self, cx),
            Section::Programs => super::programs::page(self, cx),
            Section::Plugins => super::plugins::page(self, cx),
            Section::Sessions => super::programs::sessions(self, cx),
            Section::Account => super::account::page(self, cx),
        }
    }
}

trait When: Styled + Sized {
    fn when_bg(self, on: bool, c: gpui::Hsla) -> Self {
        if on { self.bg(c) } else { self }
    }

    fn when_weight(self, on: bool) -> Self {
        if on { self.font_weight(FontWeight::SEMIBOLD) } else { self }
    }
}

impl<E: Styled> When for E {}

pub fn heading(title: &str, hint: Option<&str>, info: bool, trailing: Option<AnyElement>, cx: &App) -> AnyElement {
    let t = theme(cx);
    let title_s = title.to_owned();
    let mut row = div()
        .flex()
        .items_center()
        .gap(px(8.))
        .child(div().debug_selector(move || format!("heading:{title_s}")).text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).text_color(hsla(t.fg)).child(title.to_owned()));
    if info && let Some(h) = hint {
        row = row.child(
            div()
                .id(SharedString::from(format!("info-{title}")))
                .tooltip(wide_tip(h.to_owned(), 360.))
                .child(icon(icon::INFO_OUTLINE, 15., hsla(t.fg_faint))),
        );
    }
    row = row.child(div().flex_1()).children(trailing);
    let mut col = div().flex().flex_col().gap(px(4.)).mb(px(14.)).child(row);
    if !info && let Some(h) = hint {
        col = col.child(div().text_size(px(11.5)).line_height(px(15.5)).text_color(hsla(t.fg_faint)).child(h.to_owned()));
    }
    col.into_any_element()
}

impl Render for SettingsDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme(cx);
        let me = cx.entity().downgrade();
        let close = Rc::new(cb(me.clone(), |this, w, cx| this.act(SettingsAction::Close, w, cx)));
        let body = self.body(window, cx);
        let vp = window.viewport_size();
        let w = 800f32.min(f32::from(vp.width) - 56.);
        let h = 620f32.min(f32::from(vp.height) - 56.);
        let footer = div()
            .flex()
            .items_center()
            .px(px(16.))
            .py(px(8.))
            .border_t_1()
            .border_color(hsla(t.border))
            .child(div().flex_1().text_size(px(11.)).text_color(hsla(t.fg_faint)).child("Tudo aqui vale na hora"))
            .child(Button::new("settings-done", "Pronto", ButtonKind::Text).on_click(cb(me, |this, w, cx| this.act(SettingsAction::Close, w, cx))));
        barrier(0x0000_0033, Some(close))
            .child(
                card(&t, 12.)
                    .debug_selector(|| "settings".into())
                    .w(px(w))
                    .h(px(h))
                    .track_focus(&self.focus)
                    .on_key_down(cx.listener(Self::key_down))
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            .min_h_0()
                            .child(self.rail(cx))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .flex_1()
                                    .min_w_0()
                                    .child(div().id("settings-body").flex_1().min_h_0().overflow_y_scroll().track_scroll(&self.scroll).pl(px(24.)).pt(px(22.)).pr(px(24.)).pb(px(12.)).child(body))
                                    .child(footer),
                            ),
                    ),
            )
            .children(self.menu.clone())
    }
}
