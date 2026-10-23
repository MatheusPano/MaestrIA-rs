use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::time::SystemTime;

use gpui::{App, AppContext, Context, Entity, IntoElement, Render, Window};
use mx_store::Store;
use mx_sys::history::{ChatEntry, ChatStanding};

use crate::app::ui;
use crate::chrome::dialog::lists::{ChatHistoryDialog, ChatRow, ChatSection, More};
use crate::chrome::menu::Tone;

struct Section {
    root: Option<String>,
    name: String,
    chats: Vec<ChatEntry>,
}

pub struct ChatHistory {
    generation: u64,
    chats: Option<Vec<ChatEntry>>,
    deep: HashMap<String, Vec<ChatEntry>>,
    loading: HashSet<String>,
}

fn sections(store: &Store, window: &[ChatEntry], deep: &HashMap<String, Vec<ChatEntry>>) -> Vec<Section> {
    let claimed: HashSet<&str> = deep.values().flatten().map(|c| c.session_id.as_str()).collect();
    let mut by_root: Vec<Section> = Vec::new();
    let mut loose = Vec::new();
    for chat in window {
        let Some(folder) = store.folder_at(&chat.cwd) else {
            if !claimed.contains(chat.session_id.as_str()) {
                loose.push(chat.clone());
            }
            continue;
        };
        let at = match by_root.iter().position(|s| s.root.as_deref() == Some(folder.root.as_str())) {
            Some(i) => i,
            None => {
                by_root.push(Section { root: Some(folder.root.clone()), name: folder.name.clone(), chats: Vec::new() });
                by_root.len() - 1
            }
        };
        if !deep.contains_key(&folder.root) {
            by_root[at].chats.push(chat.clone());
        }
    }
    for s in &mut by_root {
        if let Some(d) = s.root.as_ref().and_then(|r| deep.get(r)) {
            s.chats = d.clone();
        }
    }
    if !loose.is_empty() {
        by_root.push(Section { root: None, name: store.loose().name.clone(), chats: loose });
    }
    by_root
}

fn note(standing: ChatStanding) -> Option<(String, Tone)> {
    let tone = match standing {
        ChatStanding::OnScreen => Tone::Green,
        ChatStanding::Live | ChatStanding::Gone => Tone::Yellow,
        ChatStanding::Fresh => return None,
    };
    standing.note().map(|n| (n.to_owned(), tone))
}

impl ChatHistory {
    fn load(&mut self, cx: &mut Context<Self>) {
        let host = ui::host(cx);
        let rt = host.read(cx).rt();
        let task = rt.spawn(host.read(cx).store.all_chats());
        cx.spawn(async move |this, cx| {
            let chats = task.await.unwrap_or_default();
            this.update(cx, |this, cx| {
                this.chats = Some(chats);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn deepen(&mut self, root: String, cx: &mut Context<Self>) {
        if !self.loading.insert(root.clone()) {
            return;
        }
        let host = ui::host(cx);
        let rt = host.read(cx).rt();
        let task = rt.spawn(host.read(cx).store.chats_in(&root));
        cx.spawn(async move |this, cx| {
            let chats = task.await.unwrap_or_default();
            this.update(cx, |this, cx| {
                this.loading.remove(&root);
                this.deep.insert(root, chats);
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }
}

impl Render for ChatHistory {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let store = ui::store(cx);
        let now = SystemTime::now();
        let built = self.chats.as_deref().map(|c| sections(store, c, &self.deep)).unwrap_or_default();
        let shown: Vec<ChatSection> = built
            .iter()
            .map(|s| ChatSection {
                name: s.name.clone(),
                rows: s
                    .chats
                    .iter()
                    .map(|c| {
                        let standing = store.standing_of(c);
                        ChatRow {
                            title: c.label(),
                            meta: format!("{} · {} · {}", c.ago(now), c.where_(), c.size()),
                            note: note(standing),
                            gone: standing == ChatStanding::Gone,
                        }
                    })
                    .collect(),
                more: s.root.as_ref().and_then(|root| {
                    if self.deep.contains_key(root) {
                        None
                    } else if self.loading.contains(root) {
                        Some(More::Loading)
                    } else {
                        Some(More::Offer(format!("Todas as conversas em {}", s.name)))
                    }
                }),
            })
            .collect();
        let picks: Rc<Vec<Vec<ChatEntry>>> = Rc::new(built.iter().map(|s| s.chats.clone()).collect());
        let roots: Rc<Vec<Option<String>>> = Rc::new(built.iter().map(|s| s.root.clone()).collect());
        let generation = self.generation;
        let me = cx.entity().downgrade();
        ChatHistoryDialog {
            loading: self.chats.is_none(),
            sections: shown,
            on_resume: Rc::new(move |(si, ri), _, cx| {
                let Some(chat) = picks.get(si).and_then(|s| s.get(ri)).cloned() else { return };
                ui::close_if(generation, cx);
                ui::act(cx, |s| drop(s.resume_chat(&chat, None, None)));
            }),
            on_more: Rc::new(move |si, _, cx| {
                if let Some(Some(root)) = roots.get(si).cloned() {
                    me.update(cx, |this, cx| this.deepen(root, cx)).ok();
                }
            }),
            on_close: Rc::new(move |_, cx| ui::close_if(generation, cx)),
        }
    }
}

pub fn open(cx: &mut App) -> Entity<ChatHistory> {
    let generation = ui::upcoming(cx);
    let view = cx.new(|cx| {
        let mut h = ChatHistory { generation, chats: None, deep: HashMap::new(), loading: HashSet::new() };
        h.load(cx);
        h
    });
    ui::modal(view.clone().into(), cx);
    view
}
