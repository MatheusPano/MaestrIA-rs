use mx_plugins::chord::Chord;
use mx_store::Store;

use super::chord::{LINUX, Pressed, UNUSABLE_KEY, chord_id, key_of};
use super::dispatch::keymap_of;
use super::map::Keymap;
use crate::chrome::settings::KeyPress;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HotkeyCheck {
    Ignore,
    Rejected(String),
    Accepted(String),
}

pub fn judge(p: &KeyPress, map: &Keymap, plugin_key: impl Fn(&Chord) -> Option<String>) -> HotkeyCheck {
    let key = match key_of(&p.key) {
        Pressed::Modifier => return HotkeyCheck::Ignore,
        Pressed::Unknown => return HotkeyCheck::Rejected(UNUSABLE_KEY.into()),
        Pressed::Key(k) => k,
    };
    let linux = map.linux();
    let chord = Chord { key, meta: p.meta, control: p.ctrl, alt: p.alt, shift: p.shift };
    let why = chord
        .rejection(linux)
        .or_else(|| map.owner(&chord).map(|a| format!("já é o atalho de \"{}\"", a.label())))
        .or_else(|| plugin_key(&chord).map(|title| format!("já é a tecla de \"{title}\", de um plugin")));
    match why {
        Some(why) => HotkeyCheck::Rejected(format!("{}: {why}", chord.label(linux))),
        None => HotkeyCheck::Accepted(chord_id(&chord)),
    }
}

pub fn judge_in(store: &Store, p: &KeyPress) -> HotkeyCheck {
    let map = keymap_of(store);
    judge(p, &map, |chord| {
        store
            .plugins()?
            .commands()
            .into_iter()
            .find(|c| c.key.is_some_and(|k| k == *chord || k == chord.for_host(LINUX)))
            .map(|c| c.title)
    })
}

pub fn drop_taken_hotkeys(store: &mut Store) {
    let map = keymap_of(store);
    store.retain_hotkeys(|id| Chord::parse(id).is_some_and(|c| map.owner(&c).is_none()));
}
