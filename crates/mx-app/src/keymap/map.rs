use mx_core::json::Obj;
use mx_plugins::chord::{Chord, KeyPress};
use serde_json::Value;

use super::action::Action;
use super::chord::{chord_id, is_valid};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Keymap {
    linux: bool,
    lists: Vec<Vec<Chord>>,
}

impl Keymap {
    pub fn defaults(linux: bool) -> Keymap {
        Keymap { linux, lists: Action::ALL.iter().map(|a| a.defaults(linux)).collect() }
    }

    pub fn load(saved: &Value, linux: bool) -> Keymap {
        let mut map = Keymap::defaults(linux);
        let Value::Object(obj) = saved else { return map };
        for a in Action::ALL {
            if let Some(Value::Array(items)) = obj.get(a.id()) {
                map.lists[a.index()] = items
                    .iter()
                    .filter_map(Value::as_str)
                    .filter_map(Chord::parse)
                    .filter(|c| is_valid(c, linux))
                    .collect();
            }
        }
        let mut seen: Vec<Chord> = Vec::new();
        for list in &mut map.lists {
            list.retain(|c| {
                if seen.contains(c) {
                    return false;
                }
                seen.push(*c);
                true
            });
        }
        map
    }

    pub fn from_obj(saved: &Obj, linux: bool) -> Keymap {
        Keymap::load(&Value::Object(saved.clone()), linux)
    }

    pub fn to_json(&self) -> Obj {
        let mut out = Obj::new();
        for a in Action::ALL {
            if !self.is_default(a) {
                let ids = self.chords(a).iter().map(|c| Value::String(chord_id(c))).collect();
                out.insert(a.id().to_owned(), Value::Array(ids));
            }
        }
        out
    }

    pub fn linux(&self) -> bool {
        self.linux
    }

    pub fn chords(&self, a: Action) -> &[Chord] {
        &self.lists[a.index()]
    }

    pub fn labels(&self, a: Action) -> Vec<String> {
        self.chords(a).iter().map(|c| c.label(self.linux)).collect()
    }

    pub fn first_label(&self, a: Action) -> Option<String> {
        self.chords(a).first().map(|c| c.label(self.linux))
    }

    pub fn owner(&self, c: &Chord) -> Option<Action> {
        Action::ALL.into_iter().find(|a| self.chords(*a).contains(c))
    }

    pub fn bind(&mut self, a: Action, c: Chord, replacing: Option<Chord>) -> Option<Action> {
        let previous = self.owner(&c);
        if let Some(p) = previous {
            self.lists[p.index()].retain(|x| *x != c);
        }
        let list = &mut self.lists[a.index()];
        match replacing.and_then(|r| list.iter().position(|x| *x == r)) {
            Some(i) => list[i] = c,
            None if !list.contains(&c) => list.push(c),
            None => {}
        }
        previous.filter(|p| *p != a)
    }

    pub fn unbind(&mut self, a: Action, c: &Chord) {
        self.lists[a.index()].retain(|x| x != c);
    }

    pub fn reset(&mut self) {
        *self = Keymap::defaults(self.linux);
    }

    pub fn reset_action(&mut self, a: Action) {
        let defaults = a.defaults(self.linux);
        for (i, list) in self.lists.iter_mut().enumerate() {
            if i != a.index() {
                list.retain(|c| !defaults.contains(c));
            }
        }
        self.lists[a.index()] = defaults;
    }

    pub fn is_default(&self, a: Action) -> bool {
        self.chords(a) == a.defaults(self.linux).as_slice()
    }

    pub fn all_default(&self) -> bool {
        Action::ALL.iter().all(|a| self.is_default(*a))
    }

    pub fn matching(&self, p: &KeyPress) -> Option<Action> {
        Action::ALL.into_iter().find(|a| self.chords(*a).iter().any(|c| c.accepts(p)))
    }
}
