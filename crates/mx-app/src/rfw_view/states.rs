use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;

use gpui::{App, FocusHandle, ScrollHandle, Window, WindowTextSystem};

use super::anchor;
use super::selectable::SelectionSlot;
use super::text_input_widget::TextInputs;

const SPARE_FOCUS: usize = 4;

pub type FollowLink = Rc<dyn Fn(String, &mut Window, &mut App)>;

pub struct Slots<T> {
    items: RefCell<Vec<Rc<RefCell<T>>>>,
    used: Cell<usize>,
}

impl<T> Default for Slots<T> {
    fn default() -> Self {
        Self { items: RefCell::new(Vec::new()), used: Cell::new(0) }
    }
}

impl<T: Default> Slots<T> {
    pub fn next(&self) -> Rc<RefCell<T>> {
        let i = self.used.get();
        self.used.set(i + 1);
        let mut items = self.items.borrow_mut();
        if items.len() <= i {
            items.push(Rc::new(RefCell::new(T::default())));
        }
        items[i].clone()
    }

    pub fn peek(&self) -> Rc<RefCell<T>> {
        let i = self.used.get();
        let mut items = self.items.borrow_mut();
        if items.len() <= i {
            items.push(Rc::new(RefCell::new(T::default())));
        }
        items[i].clone()
    }

    pub fn each(&self, mut f: impl FnMut(&mut T)) {
        for item in self.items.borrow().iter() {
            f(&mut item.borrow_mut());
        }
    }

    fn settle(&self) {
        self.items.borrow_mut().truncate(self.used.get());
        self.used.set(0);
    }
}

pub struct WidgetStates {
    pub owner: u64,
    pub plugin_id: String,
    pub follow: Option<FollowLink>,
    pub inputs: TextInputs,
    pub sprites: Slots<super::sprite::SpriteState>,
    pub zooms: Slots<super::zoom_view::ZoomState>,
    pub fits: Slots<super::zoom_view::FitState>,
    pub selections: Slots<super::selectable::SelectionState>,
    pub anchors: RefCell<HashSet<String>>,
    pub current_selection: RefCell<Option<SelectionSlot>>,
    pub text_system: RefCell<Option<Arc<WindowTextSystem>>>,
    pub keys: Slots<super::key_listener::KeyListenerState>,
    pub key_scope: RefCell<Option<super::key_listener::KeySlot>>,
    pub scrolls: RefCell<HashMap<String, ScrollHandle>>,
    spare_focus: RefCell<Vec<FocusHandle>>,
}

impl Default for WidgetStates {
    fn default() -> Self {
        Self {
            owner: anchor::new_owner(),
            plugin_id: String::new(),
            follow: None,
            inputs: TextInputs::default(),
            sprites: Slots::default(),
            zooms: Slots::default(),
            fits: Slots::default(),
            selections: Slots::default(),
            anchors: RefCell::new(HashSet::new()),
            current_selection: RefCell::new(None),
            text_system: RefCell::new(None),
            keys: Slots::default(),
            key_scope: RefCell::new(None),
            scrolls: RefCell::new(HashMap::new()),
            spare_focus: RefCell::new(Vec::new()),
        }
    }
}

impl WidgetStates {
    pub fn begin(&self, window: &Window, app: &App) {
        let mut spare = self.spare_focus.borrow_mut();
        while spare.len() < SPARE_FOCUS {
            spare.push(app.focus_handle());
        }
        drop(spare);
        self.key_scope.borrow_mut().take();
        self.anchors.borrow_mut().clear();
        self.current_selection.borrow_mut().take();
        self.text_system.replace(Some(window.text_system().clone()));
    }

    pub fn finish(&self) {
        self.sprites.settle();
        self.zooms.settle();
        self.fits.settle();
        self.selections.settle();
        self.keys.settle();
        anchor::forget_unseen(self.owner, &self.anchors.borrow());
    }

    pub fn release_space(&self) {
        self.zooms.each(super::zoom_view::ZoomState::release_space);
        self.keys.each(super::key_listener::KeyListenerState::release_space);
    }

    pub fn take_focus(&self) -> Option<FocusHandle> {
        self.spare_focus.borrow_mut().pop()
    }

    pub fn scroll(&self, id: &str) -> ScrollHandle {
        self.scrolls.borrow_mut().entry(id.to_owned()).or_default().clone()
    }

    pub fn selection(&self) -> Option<SelectionSlot> {
        self.current_selection.borrow().clone()
    }
}

impl Drop for WidgetStates {
    fn drop(&mut self) {
        anchor::forget_owner(self.owner);
    }
}
