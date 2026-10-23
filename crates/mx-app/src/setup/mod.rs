pub mod demo;
mod editor;
mod overlay;
mod state;
mod view;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_support;

use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use gpui::{
    App, AppContext, Context, Entity, FocusHandle, Focusable, ScrollHandle, Subscription, Task,
    Window,
};
use mx_claude::setup::{self, MxSetup, SetupSection, SetupSlot};
use mx_theme::Theme;

pub use editor::{EditorEvent, TextEditor};
use editor::EditorStyle;
use overlay::{Overlay, Pending};
use state::{DocState, Poll, slot_for};

const PULSE: Duration = Duration::from_millis(1500);

pub type Action = Rc<dyn Fn(&mut Window, &mut App)>;
pub type PathAction = Rc<dyn Fn(String, &mut Window, &mut App)>;
pub type BannerAction = Rc<dyn Fn(String, bool, &mut Window, &mut App)>;
pub type SetupChanged = Rc<dyn Fn(MxSetup, &mut Window, &mut App)>;

#[derive(Clone, Default)]
pub struct SetupActions {
    pub banner: Option<BannerAction>,
    pub open_in_editor: Option<PathAction>,
    pub reveal: Option<PathAction>,
    pub close: Option<Action>,
    pub toggle_pin: Option<Action>,
    pub changed: Option<SetupChanged>,
}

pub struct SetupView {
    setup: MxSetup,
    showing: String,
    slots: Vec<SetupSlot>,
    doc: DocState,
    editor: Entity<TextEditor>,
    theme: Arc<Theme>,
    zoom: f32,
    title: Option<String>,
    pinned: bool,
    header: bool,
    actions: SetupActions,
    overlay: Option<Overlay>,
    list_scroll: ScrollHandle,
    welcome_scroll: ScrollHandle,
    focus: FocusHandle,
    poll: Option<Task<()>>,
    _editor_events: Subscription,
}

impl Focusable for SetupView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl SetupView {
    pub fn new(setup: MxSetup, theme: Arc<Theme>, actions: SetupActions, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let editor = cx.new(|cx| TextEditor::new("", cx));
        let events = cx.subscribe_in(&editor, window, |this, _, ev: &EditorEvent, window, cx| match ev {
            EditorEvent::Changed => cx.notify(),
            EditorEvent::Save => this.save(window, cx),
        });
        let mut view = Self {
            setup,
            showing: String::new(),
            slots: Vec::new(),
            doc: DocState::default(),
            editor,
            theme,
            zoom: 0.,
            title: None,
            pinned: false,
            header: true,
            actions,
            overlay: None,
            list_scroll: ScrollHandle::new(),
            welcome_scroll: ScrollHandle::new(),
            focus: cx.focus_handle(),
            poll: None,
            _editor_events: events,
        };
        view.sync_style(cx);
        view.adopt(cx);
        view
    }

    pub fn setup(&self) -> &MxSetup {
        &self.setup
    }

    pub fn text(&self, cx: &App) -> String {
        self.editor.read(cx).text().to_owned()
    }

    pub fn editor(&self) -> &Entity<TextEditor> {
        &self.editor
    }

    pub fn set_setup(&mut self, setup: MxSetup, cx: &mut Context<Self>) {
        self.setup = setup;
        self.adopt(cx);
    }

    pub fn set_theme(&mut self, theme: Arc<Theme>, cx: &mut Context<Self>) {
        self.theme = theme;
        self.sync_style(cx);
        cx.notify();
    }

    pub fn set_zoom(&mut self, zoom: f32, cx: &mut Context<Self>) {
        self.zoom = zoom;
        self.sync_style(cx);
        cx.notify();
    }

    pub fn set_title(&mut self, title: Option<String>, cx: &mut Context<Self>) {
        self.title = title;
        cx.notify();
    }

    pub fn set_pinned(&mut self, pinned: bool, cx: &mut Context<Self>) {
        self.pinned = pinned;
        cx.notify();
    }

    pub fn set_header(&mut self, shown: bool, cx: &mut Context<Self>) {
        self.header = shown;
        cx.notify();
    }

    pub fn title(&self) -> String {
        self.title.clone().unwrap_or_else(|| format!("Claude · {}", self.setup.name()))
    }

    fn sync_style(&mut self, cx: &mut Context<Self>) {
        let t = &self.theme;
        let style = EditorStyle {
            font_size: 12.5 + self.zoom,
            line_height: 1.55,
            fg: view::hsla(t.fg),
            caret: view::hsla(t.accent),
            selection: view::hsla(t.accent.with_alpha(0.3)),
            thumb: view::hsla(t.fg_faint.with_alpha(0.35)),
        };
        self.editor.update(cx, |e, cx| e.set_style(style, cx));
    }

    fn adopt(&mut self, cx: &mut Context<Self>) {
        if self.setup.root == self.showing {
            return;
        }
        self.showing = self.setup.root.clone();
        self.slots = setup::scan(&self.setup.root);
        self.load(cx);
        self.start_poll(cx);
    }

    fn start_poll(&mut self, cx: &mut Context<Self>) {
        self.poll = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(PULSE).await;
                let Ok((root, rel)) = this.update(cx, |v, _| (v.setup.root.clone(), v.setup.selected.clone())) else {
                    break;
                };
                let (slots, now) = cx
                    .background_executor()
                    .spawn(async move {
                        let now = rel.as_ref().map(|r| (r.clone(), setup::read(&root, r)));
                        (setup::scan(&root), now)
                    })
                    .await;
                if this.update(cx, |v, cx| v.tick(slots, now, cx)).is_err() {
                    break;
                }
            }
        }));
    }

    pub(crate) fn tick(&mut self, slots: Vec<SetupSlot>, now: Option<(String, String)>, cx: &mut Context<Self>) {
        let mut changed = !state::same_list(&slots, &self.slots);
        self.slots = slots;
        let now = now.filter(|(rel, _)| Some(rel) == self.setup.selected.as_ref()).map(|(_, now)| now);
        if let Some(now) = now {
            let text = self.text(cx);
            match self.doc.poll(now, &text) {
                Poll::Same => {}
                Poll::Outside => changed = true,
                Poll::Replace(fresh) => {
                    self.editor.update(cx, |e, cx| e.set_text(fresh, cx));
                    changed = true;
                }
            }
        }
        if changed {
            cx.notify();
        }
    }

    pub fn slot(&self) -> Option<SetupSlot> {
        let rel = self.setup.selected.as_ref()?;
        Some(slot_for(&self.slots, &self.setup.root, rel))
    }

    fn edited(&self, cx: &App) -> bool {
        self.doc.edited(self.editor.read(cx).text())
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        let slot = self.slot();
        self.doc = DocState::load(slot.as_ref(), &self.setup.root, self.setup.name());
        let text = self.doc.loaded.clone();
        self.editor.update(cx, |e, cx| e.set_text(text, cx));
        cx.notify();
    }

    fn banner(&self, text: String, sticky: bool, window: &mut Window, cx: &mut App) {
        match &self.actions.banner {
            Some(b) => b(text, sticky, window, cx),
            None => log::info!("setup banner: {text}"),
        }
    }

    fn announce(&self, window: &mut Window, cx: &mut App) {
        if let Some(f) = self.actions.changed.clone() {
            f(self.setup.clone(), window, cx);
        }
    }

    fn focus_editor(&self, window: &mut Window, cx: &App) {
        self.editor.read(cx).focus(window);
    }

    pub fn select(&mut self, rel: String, window: &mut Window, cx: &mut Context<Self>) {
        if Some(&rel) == self.setup.selected.as_ref() {
            self.focus_editor(window, cx);
            return;
        }
        if self.edited(cx) && self.slot().is_some() {
            self.ask_discard(Pending::Select(rel), cx);
            return;
        }
        self.select_now(rel, window, cx);
    }

    fn select_now(&mut self, rel: String, window: &mut Window, cx: &mut Context<Self>) {
        self.setup.selected = Some(rel);
        self.announce(window, cx);
        self.load(cx);
        self.focus_editor(window, cx);
    }

    pub fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(slot) = self.slot() else {
            return;
        };
        let text = self.text(cx);
        if let Err(e) = setup::write(&self.setup.root, &slot.rel, &text) {
            self.banner(format!("Não consegui salvar {}: {e}", slot.rel), true, window, cx);
            return;
        }
        self.doc.saved(&text);
        self.slots = setup::scan(&self.setup.root);
        self.banner(format!("{} salvo", slot.label), false, window, cx);
        cx.notify();
    }

    pub fn revert(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.edited(cx) {
            self.ask_discard(Pending::Revert, cx);
            return;
        }
        self.load(cx);
        self.focus_editor(window, cx);
    }

    pub fn add(&mut self, section: SetupSection, window: &mut Window, cx: &mut Context<Self>) {
        self.open_prompt(section, window, cx);
    }

    fn named(&mut self, section: SetupSection, name: String, window: &mut Window, cx: &mut Context<Self>) {
        if name.trim().is_empty() {
            return;
        }
        let Some(rel) = setup::rel_for(section, &name, &self.setup.root) else {
            self.banner("Esse nome não sobra nada depois de virar arquivo".into(), false, window, cx);
            return;
        };
        if std::path::Path::new(&setup::path_of(&self.setup.root, &rel)).exists() {
            self.banner(format!("Já existe: {rel}"), false, window, cx);
            self.select(rel, window, cx);
            return;
        }
        let create = Pending::Create { rel, name: name.trim().to_owned() };
        if self.edited(cx) && self.slot().is_some() {
            self.ask_discard(create, cx);
            return;
        }
        self.resume(create, window, cx);
    }

    fn create_now(&mut self, rel: String, name: String, window: &mut Window, cx: &mut Context<Self>) {
        let body = setup::template(&rel, self.setup.name(), &name);
        if let Err(e) = setup::write(&self.setup.root, &rel, &body) {
            self.banner(format!("Não consegui criar {rel}: {e}"), true, window, cx);
            return;
        }
        self.slots = setup::scan(&self.setup.root);
        self.select_now(rel, window, cx);
    }

    pub fn delete(&mut self, cx: &mut Context<Self>) {
        if let Some(slot) = self.slot().filter(|s| s.exists) {
            self.overlay = Some(Overlay::Delete(slot));
            cx.notify();
        }
    }

    fn delete_now(&mut self, slot: SetupSlot, window: &mut Window, cx: &mut Context<Self>) {
        if let Err(e) = setup::remove(&self.setup.root, &slot.rel) {
            self.banner(format!("Não consegui apagar {}: {e}", slot.rel), true, window, cx);
            return;
        }
        self.slots = setup::scan(&self.setup.root);
        if !slot.fixed {
            self.setup.selected = None;
            self.announce(window, cx);
        }
        self.load(cx);
    }

    fn resume(&mut self, pending: Pending, window: &mut Window, cx: &mut Context<Self>) {
        match pending {
            Pending::Select(rel) => self.select_now(rel, window, cx),
            Pending::Revert => {
                self.load(cx);
                self.focus_editor(window, cx);
            }
            Pending::Create { rel, name } => self.create_now(rel, name, window, cx),
        }
    }
}
