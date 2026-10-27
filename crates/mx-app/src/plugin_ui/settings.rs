use std::collections::{HashMap, HashSet};

use gpui::{
    AnyElement, AppContext, Context, Entity, FontWeight, InteractiveElement, IntoElement, ParentElement, Render,
    StatefulInteractiveElement, Styled, Subscription, Window, div, prelude::FluentBuilder, px,
};
use mx_plugins::{PluginSetting, SettingType};
use serde_json::Value;

use super::widgets::{Close, barrier, card, chip, text_button, title};
use crate::app::host::{StoreHost, update_store};
use crate::chrome::dialog::frame::BARRIER;
use crate::chrome::style::{hsla, theme};
use super::folder_picker::{FolderPicker, Pick, button_text};
use crate::chrome::icon::{self, icon};
use crate::chrome::widgets::{MxSwitch, TextField, TextFieldEvent, wide_tip};

pub const PRESETS: [&str; 6] = ["#4FC1FF", "#89D185", "#F48771", "#CCA700", "#C586C0", "#9DA5B4"];
const NAMED: [&str; 7] = ["red", "green", "yellow", "accent", "purple", "faint", "dim"];

pub fn color_ok(text: &str) -> bool {
    let t = text.trim();
    let hex = |s: &str| s.chars().all(|c| c.is_ascii_hexdigit());
    t.is_empty()
        || NAMED.contains(&t)
        || t.strip_prefix('#').is_some_and(|h| (h.len() == 6 || h.len() == 8) && hex(h))
}

pub fn number_of(text: &str) -> Option<Value> {
    let t = text.trim();
    if let Ok(i) = t.parse::<i64>() {
        return Some(Value::from(i));
    }
    t.parse::<f64>().ok().filter(|f| f.is_finite()).map(Value::from)
}

pub fn settings_title(name: &str) -> String {
    format!("Configurar {name}")
}

fn shown(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

pub struct SettingsDialog {
    host: Entity<StoreHost>,
    plugin: String,
    close: Close,
    fields: HashMap<String, Entity<TextField>>,
    invalid: HashSet<String>,
    picker: Option<Entity<FolderPicker>>,
    _subs: Vec<Subscription>,
}

impl SettingsDialog {
    pub fn new(host: Entity<StoreHost>, plugin: String, close: Close, _: &mut Window, cx: &mut Context<Self>) -> Self {
        let (settings, saved) = Self::read(&host, &plugin, cx);
        let mut fields = HashMap::new();
        let mut subs = Vec::new();
        for s in settings.iter().filter(|s| matches!(s.kind, SettingType::String | SettingType::Number | SettingType::Color)) {
            let text = saved.get(&s.id).map(shown).unwrap_or_default();
            let hint = if s.kind == SettingType::Color { "Do tema" } else { "" };
            let e = cx.new(|cx| TextField::new(cx).hint(hint).filled().with_text(text));
            let (id, kind) = (s.id.clone(), s.kind);
            subs.push(cx.subscribe(&e, move |this: &mut Self, _, ev: &TextFieldEvent, cx| {
                if let TextFieldEvent::Changed(text) = ev {
                    this.typed(&id, kind, text, cx);
                }
            }));
            fields.insert(s.id.clone(), e);
        }
        SettingsDialog { host, plugin, close, fields, invalid: HashSet::new(), picker: None, _subs: subs }
    }

    fn read(host: &Entity<StoreHost>, plugin: &str, cx: &gpui::App) -> (Vec<PluginSetting>, serde_json::Map<String, Value>) {
        let Some(r) = host.read(cx).store.plugins() else { return Default::default() };
        let settings = r.get(plugin).and_then(|p| p.manifest.clone()).map(|m| m.settings.clone()).unwrap_or_default();
        (settings, r.settings_of(plugin))
    }

    fn save(&self, key: &str, value: Option<Value>, cx: &mut gpui::App) {
        let plugin = self.plugin.clone();
        update_store(&self.host, cx, |s| s.set_plugin_setting(&plugin, key, value));
    }

    fn typed(&mut self, id: &str, kind: SettingType, text: &str, cx: &mut Context<Self>) {
        let value = match kind {
            SettingType::Number => number_of(text),
            SettingType::Color => color_ok(text).then(|| Value::from(text.trim())),
            _ => Some(Value::from(text)),
        };
        match value {
            Some(v) => {
                self.invalid.remove(id);
                self.save(id, Some(v), cx);
            }
            None => {
                self.invalid.insert(id.to_owned());
            }
        }
        cx.notify();
    }

    fn reset(&mut self, s: &PluginSetting, cx: &mut Context<Self>) {
        self.save(&s.id, None, cx);
        self.invalid.remove(&s.id);
        if let Some(e) = self.fields.get(&s.id) {
            let text = shown(&s.resolve(None));
            e.update(cx, |f, cx| f.set_text(text, cx));
        }
        cx.notify();
    }

    fn control(&self, s: &PluginSetting, value: &Value, cx: &mut Context<Self>) -> AnyElement {
        let t = theme(cx);
        let me = cx.weak_entity();
        match s.kind {
            SettingType::Boolean => {
                let (id, on) = (s.id.clone(), value.as_bool().unwrap_or(false));
                MxSwitch::new(gpui::SharedString::from(format!("set-{}", s.id)), on)
                    .on_change(move |v, _, cx| {
                        me.update(cx, |d, cx| d.save(&id, Some(Value::Bool(v)), cx)).ok();
                    })
                    .into_any_element()
            }
            SettingType::Folders => self.folders_button(s, value, cx),
            SettingType::Select => {
                let current = value.as_str().unwrap_or_default().to_owned();
                let mut chips = div().flex().flex_wrap().gap(px(6.));
                for o in &s.options {
                    let (me, id, v) = (me.clone(), s.id.clone(), o.value.clone());
                    chips = chips.child(chip(format!("set-{}={}", s.id, o.value), o.label.clone(), o.value == current, &t, move |_, cx| {
                        me.update(cx, |d, cx| d.save(&id, Some(Value::from(v.clone())), cx)).ok();
                    }));
                }
                chips.into_any_element()
            }
            SettingType::Color => {
                let mut row = div().flex().items_center().gap(px(6.));
                for p in PRESETS {
                    let color = mx_theme::Color::parse_hex(p).map(hsla).unwrap_or_default();
                    let (me, id) = (me.clone(), s.id.clone());
                    row = row.child(
                        div()
                            .id(gpui::ElementId::Name(format!("{}-{p}", s.id).into()))
                            .size(px(18.))
                            .rounded(px(9.))
                            .bg(color)
                            .cursor_pointer()
                            .on_click(move |_, _, cx| {
                                me.update(cx, |d, cx| {
                                    d.save(&id, Some(Value::from(p)), cx);
                                    if let Some(e) = d.fields.get(&id) {
                                        e.update(cx, |f, cx| f.set_text(p, cx));
                                    }
                                })
                                .ok();
                            }),
                    );
                }
                let sample = mx_theme::Color::parse_hex(value.as_str().unwrap_or_default()).map(hsla).unwrap_or(hsla(t.fg));
                row = row.child(div().text_size(px(12.)).text_color(sample).font_family("Hack").child("Aa log"));
                div().flex().flex_col().gap(px(6.)).child(row).children(self.field(&s.id, cx)).into_any_element()
            }
            _ => div().children(self.field(&s.id, cx)).into_any_element(),
        }
    }

    fn field(&self, id: &str, cx: &mut Context<Self>) -> Option<AnyElement> {
        let t = theme(cx);
        let e = self.fields.get(id)?.clone();
        let bad = self.invalid.contains(id);
        Some(div().when(bad, |d| d.border_1().border_color(hsla(t.red)).rounded(px(8.))).child(e).into_any_element())
    }

    fn label(&self, s: &PluginSetting, cx: &mut Context<Self>) -> gpui::Div {
        let t = theme(cx);
        let mut row = div().flex().items_center().min_w_0().child(
            div().min_w_0().text_size(px(13.)).font_weight(FontWeight::MEDIUM).text_color(hsla(t.fg)).child(s.title.clone()),
        );
        if !s.description.is_empty() {
            row = row.child(
                div()
                    .id(gpui::SharedString::from(format!("help-{}", s.id)))
                    .ml(px(6.))
                    .flex_none()
                    .tooltip(wide_tip(s.description.clone(), 360.))
                    .child(icon(icon::HELP_OUTLINE, 14., hsla(t.fg_faint))),
            );
        }
        row
    }

    fn folders_button(&self, s: &PluginSetting, value: &Value, cx: &mut Context<Self>) -> AnyElement {
        let t = theme(cx);
        let folders = self.sidebar_folders(cx);
        let picked = picked_roots(value);
        let (text, faint) = button_text(&folders, &picked);
        let enabled = !folders.is_empty();
        let setting = s.clone();
        let mut b = div()
            .id(gpui::SharedString::from(format!("folders-{}", s.id)))
            .debug_selector({
                let text = text.clone();
                move || format!("folders-button:{text}")
            })
            .w(px(200.))
            .min_h(px(32.))
            .px(px(10.))
            .py(px(8.))
            .flex()
            .items_center()
            .gap(px(6.))
            .rounded(px(6.))
            .border_1()
            .border_color(hsla(t.border))
            .child(icon(icon::FOLDER_OUTLINED, 14., hsla(t.fg_dim)))
            .child(div().flex_1().min_w_0().truncate().text_size(px(12.5)).text_color(hsla(if faint { t.fg_faint } else { t.fg })).child(text))
            .child(icon(icon::UNFOLD_MORE, 14., hsla(t.fg_faint)));
        if enabled {
            b = b.cursor_pointer().hover(|st| st.bg(hsla(t.bg_hover))).on_click(cx.listener(move |d, _, window, cx| {
                d.pick_folders(&setting, window, cx);
            }));
        } else {
            b = b.opacity(0.6);
        }
        b.into_any_element()
    }

    fn sidebar_folders(&self, cx: &gpui::App) -> Vec<Pick> {
        self.host.read(cx).store.folders().iter().map(|f| Pick { root: f.root.clone(), name: f.name.clone() }).collect()
    }

    pub fn pick_folders(&mut self, s: &PluginSetting, window: &mut Window, cx: &mut Context<Self>) {
        let (_, saved) = Self::read(&self.host, &self.plugin, cx);
        let value = saved.get(&s.id).cloned().unwrap_or_else(|| s.resolve(None));
        let folders = self.sidebar_folders(cx);
        let me = cx.weak_entity();
        let setting = s.clone();
        let done: super::folder_picker::Done = std::rc::Rc::new(move |result: Option<Vec<String>>, _, cx: &mut gpui::App| {
            let setting = setting.clone();
            me.update(cx, |d, cx| {
                d.picker = None;
                if let Some(roots) = result {
                    let next = Value::Array(roots.into_iter().map(Value::String).collect());
                    let stored = (next != setting.resolve(None)).then_some(next);
                    d.save(&setting.id, stored, cx);
                }
                cx.notify();
            })
            .ok();
        });
        let title = s.title.clone();
        self.picker = Some(cx.new(|cx| FolderPicker::new(title, folders, picked_roots(&value), done, window, cx)));
        cx.notify();
    }

    fn row(&self, s: &PluginSetting, value: &Value, cx: &mut Context<Self>) -> AnyElement {
        let t = theme(cx);
        let mut col = div().flex().flex_col().gap(px(4.)).py(px(8.));
        let inline = matches!(s.kind, SettingType::Boolean | SettingType::Folders);
        col = if inline {
            col.child(
                div()
                    .flex()
                    .items_center()
                    .child(self.label(s, cx).flex_1())
                    .child(div().w(px(12.)))
                    .child(self.control(s, value, cx)),
            )
        } else {
            col.child(self.label(s, cx)).child(self.control(s, value, cx))
        };
        if *value != s.resolve(None) {
            let me = cx.weak_entity();
            let setting = s.clone();
            col = col.child(div().flex().child(text_button(format!("reset-{}", s.id), "Voltar ao padrão", hsla(t.accent), None, move |_, cx| {
                me.update(cx, |d, cx| d.reset(&setting, cx)).ok();
            })));
        }
        col.into_any_element()
    }

    fn shortcuts(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let t = theme(cx);
        let store = &self.host.read(cx).store;
        let commands: Vec<_> = store.plugins()?.commands().into_iter().filter(|c| c.plugin_id == self.plugin && c.key.is_some()).collect();
        if commands.is_empty() {
            return None;
        }
        let linux = crate::keymap::LINUX;
        let keymap = crate::keymap::keymap_of(store);
        let mut col = div().flex().flex_col().gap(px(4.)).pt(px(12.)).child(title(&t, "Atalhos"));
        for c in commands {
            let Some(chord) = c.key.map(|k| k.for_host(linux)) else { continue };
            let taken = keymap.owner(&chord);
            let mut line = div().flex().gap(px(10.)).text_size(px(12.5)).child(div().flex_1().text_color(hsla(t.fg)).child(c.title.clone()));
            let label = div().font_family("Hack").text_color(hsla(t.fg_dim)).child(chord.label(linux));
            line = line.child(if taken.is_some() { label.line_through() } else { label });
            col = col.child(line);
            if let Some(a) = taken {
                let note = format!("Já é \"{}\" no app — este comando fica sem tecla", a.label());
                col = col.child(div().text_size(px(11.)).text_color(hsla(t.yellow)).child(note));
            }
        }
        Some(col.into_any_element())
    }
}

impl Render for SettingsDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme(cx);
        let (settings, saved) = Self::read(&self.host, &self.plugin, cx);
        let name = self.host.read(cx).store.plugins().and_then(|r| r.get(&self.plugin)).map_or_else(|| self.plugin.clone(), |p| p.name().to_owned());
        let mut body = div().id("plugin-settings").max_h(px(520.)).overflow_y_scroll().px(px(24.)).flex().flex_col();
        let mut group: Option<String> = None;
        for s in &settings {
            if s.group.is_some() && s.group != group {
                group = s.group.clone();
                body = body.child(div().pt(px(10.)).text_size(px(11.)).font_weight(FontWeight::BOLD).text_color(hsla(t.fg_faint)).child(group.clone().unwrap_or_default()));
            }
            let value = saved.get(&s.id).cloned().unwrap_or_else(|| s.resolve(None));
            body = body.child(self.row(s, &value, cx));
        }
        body = body.children(self.shortcuts(cx));
        let picker = self.picker.clone();
        let (close, dismiss) = (self.close.clone(), self.close.clone());
        barrier(BARRIER, move |w, cx| dismiss(w, cx)).flex().items_center().justify_center().child(
            card(&t, 520.)
                .child(div().px(px(24.)).pt(px(18.)).pb(px(6.)).child(title(&t, settings_title(&name))))
                .child(body)
                .child(
                    div()
                        .px(px(16.))
                        .py(px(12.))
                        .flex()
                        .items_center()
                        .child(div().pl(px(8.)).text_size(px(11.5)).text_color(hsla(t.fg_faint)).child("Vale na hora"))
                        .child(div().flex_1())
                        .child(text_button("settings-done", "Pronto", hsla(t.on_accent()), Some(hsla(t.accent)), move |w, cx| close(w, cx))),
                ),
        )
        .children(picker)
    }
}

fn picked_roots(value: &Value) -> Vec<String> {
    value.as_array().map(|a| a.iter().filter_map(Value::as_str).map(str::to_owned).collect()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_color_is_empty_hex_or_a_theme_name() {
        assert!(color_ok(""));
        assert!(color_ok("#4FC1FF"));
        assert!(color_ok("#804FC1FF"));
        assert!(color_ok("accent"));
        assert!(!color_ok("#4FC1F"));
        assert!(!color_ok("azul"));
    }

    #[test]
    fn a_number_setting_takes_only_numbers() {
        assert_eq!(number_of("25"), Some(Value::from(25)));
        assert_eq!(number_of("2.5"), Some(Value::from(2.5)));
        assert_eq!(number_of("abc"), None);
        assert_eq!(settings_title("Pomodoro"), "Configurar Pomodoro");
    }
}
