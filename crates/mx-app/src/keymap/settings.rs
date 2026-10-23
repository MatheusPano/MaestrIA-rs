use mx_plugins::chord::Chord;
use mx_store::Store;

use super::action::Action;
use super::chord::chord_id;
use super::dispatch::keymap_of;
use super::map::Keymap;
use crate::chrome::settings::{SettingsAction, ShortcutRow};

pub fn shortcut_rows(map: &Keymap) -> Vec<ShortcutRow> {
    Action::ALL
        .into_iter()
        .map(|a| ShortcutRow {
            action: a.id().to_owned(),
            label: a.label().to_owned(),
            hint: a.hint().map(str::to_owned),
            group: a.group().to_owned(),
            chords: map.chords(a).iter().map(|c| (chord_id(c), c.label(map.linux()))).collect(),
            changed: !map.is_default(a),
        })
        .collect()
}

pub fn zoom_keys(map: &Keymap) -> (String, String) {
    let first = |a| map.first_label(a).unwrap_or_default();
    (first(Action::ZoomIn), first(Action::ZoomOut))
}

pub fn stolen_banner(chord: &Chord, from: Action, linux: bool) -> String {
    format!("{} saiu de \"{}\"", chord.label(linux), from.label())
}

pub fn edit(map: &mut Keymap, action: &SettingsAction) -> Option<Option<String>> {
    let linux = map.linux();
    match action {
        SettingsAction::ResetShortcuts(None) => map.reset(),
        SettingsAction::ResetShortcuts(Some(id)) => map.reset_action(Action::by_id(id)?),
        SettingsAction::Bind { action, chord, replacing } => {
            let a = Action::by_id(action)?;
            let c = Chord::parse(chord).filter(|c| c.rejection(linux).is_none())?;
            let replacing = replacing.as_deref().and_then(Chord::parse);
            let stolen = map.bind(a, c, replacing);
            return Some(stolen.map(|from| stolen_banner(&c, from, linux)));
        }
        SettingsAction::Unbind { action, chord } => {
            let a = Action::by_id(action)?;
            let c = Chord::parse(chord)?;
            if map.chords(a).len() > 1 {
                map.unbind(a, &c);
            }
        }
        _ => return None,
    }
    Some(None)
}

pub fn apply(store: &mut Store, action: &SettingsAction) -> bool {
    let mut map = keymap_of(store);
    let Some(banner) = edit(&mut map, action) else { return false };
    store.set_shortcuts(map.to_json());
    if let Some(text) = banner {
        store.show_banner(&text);
    }
    true
}
