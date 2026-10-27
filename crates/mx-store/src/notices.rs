use mx_core::model::notice::{NOTICE_CAP, TOAST_CAP};
use mx_core::model::{Notice, NoticeKind};
use mx_core::text::Time;

use crate::changes::Areas;
use crate::store::Store;
use crate::timers::{BANNER_LIFE, TOAST_LIFE, TimerKey};

#[derive(Debug, Clone, PartialEq)]
pub struct NoticeEntry {
    pub id: u64,
    pub notice: Notice,
}

#[derive(Debug)]
pub struct Bell {
    pub banner: Option<String>,
    pub notices: Vec<NoticeEntry>,
    pub toasts: Vec<u64>,
    pub held: Vec<u64>,
    pub notices_open: bool,
    pub sessions_open: bool,
    pub window_active: bool,
    pub away_since: Option<Time>,
    next: u64,
}

impl Default for Bell {
    fn default() -> Self {
        Bell {
            banner: None,
            notices: Vec::new(),
            toasts: Vec::new(),
            held: Vec::new(),
            notices_open: false,
            sessions_open: false,
            window_active: true,
            away_since: None,
            next: 0,
        }
    }
}

impl Store {
    pub fn bell(&self) -> &Bell {
        &self.bell
    }

    pub fn banner(&self) -> Option<&str> {
        self.bell.banner.as_deref()
    }

    pub fn notices(&self) -> &[NoticeEntry] {
        &self.bell.notices
    }

    pub fn notice(&self, id: u64) -> Option<&NoticeEntry> {
        self.bell.notices.iter().find(|n| n.id == id)
    }

    pub fn toasts(&self) -> Vec<&NoticeEntry> {
        self.bell.toasts.iter().filter_map(|id| self.notice(*id)).collect()
    }

    pub fn unread_notices(&self) -> usize {
        self.bell.notices.iter().filter(|n| !n.notice.read).count()
    }

    pub fn show_banner(&mut self, text: &str) {
        self.show_banner_with(text, false);
    }

    pub fn show_banner_sticky(&mut self, text: &str) {
        self.show_banner_with(text, true);
    }

    pub fn show_banner_with(&mut self, text: &str, sticky: bool) {
        self.bell.banner = Some(text.to_owned());
        self.timers.cancel(&TimerKey::Banner);
        if !sticky {
            let at = self.now() + BANNER_LIFE;
            self.timers.set(TimerKey::Banner, at);
        }
        self.mark(Areas::BANNER);
    }

    pub fn clear_banner(&mut self) {
        self.timers.cancel(&TimerKey::Banner);
        if self.bell.banner.take().is_some() {
            self.mark(Areas::BANNER);
        }
    }

    pub(crate) fn push_notice(&mut self, tab: &str, kind: NoticeKind, suggestion: Option<String>, detail: Option<String>) -> u64 {
        if kind != NoticeKind::Suggested {
            self.read_notices_of(tab, false, true);
        }
        let read = self.watching(tab);
        let id = self.bell.next;
        self.bell.next += 1;
        let notice = Notice {
            tab_id: tab.to_owned(),
            kind,
            title: self.title_of(tab),
            at: self.now(),
            suggestion_id: suggestion,
            detail,
            read,
        };
        self.bell.notices.insert(0, NoticeEntry { id, notice });
        while self.bell.notices.len() > NOTICE_CAP {
            if let Some(gone) = self.bell.notices.pop() {
                self.drop_toast(gone.id);
            }
        }
        if !read && !self.prefs.do_not_disturb && !self.bell.notices_open {
            self.toast(id);
        }
        self.mark(Areas::NOTICES);
        id
    }

    fn toast(&mut self, id: u64) {
        self.bell.toasts.push(id);
        while self.bell.toasts.len() > TOAST_CAP {
            let gone = self.bell.toasts.remove(0);
            self.timers.cancel(&TimerKey::Toast(gone));
        }
        self.arm_toast(id);
    }

    fn arm_toast(&mut self, id: u64) {
        self.timers.cancel(&TimerKey::Toast(id));
        if self.bell.window_active {
            let at = self.now() + TOAST_LIFE;
            self.timers.set(TimerKey::Toast(id), at);
        }
    }

    fn drop_toast(&mut self, id: u64) -> bool {
        self.timers.cancel(&TimerKey::Toast(id));
        self.bell.held.retain(|h| *h != id);
        let before = self.bell.toasts.len();
        self.bell.toasts.retain(|t| *t != id);
        before != self.bell.toasts.len()
    }

    pub fn dismiss_toast(&mut self, id: u64) {
        if self.drop_toast(id) {
            self.mark(Areas::NOTICES);
        }
    }

    pub fn hold_toast(&mut self, id: u64, hold: bool) {
        if hold {
            self.timers.cancel(&TimerKey::Toast(id));
            if !self.bell.held.contains(&id) {
                self.bell.held.push(id);
            }
        } else {
            self.bell.held.retain(|h| *h != id);
            if self.bell.toasts.contains(&id) {
                self.arm_toast(id);
            }
        }
    }

    pub(crate) fn clear_toasts(&mut self) {
        for id in std::mem::take(&mut self.bell.toasts) {
            self.timers.cancel(&TimerKey::Toast(id));
        }
        self.bell.held.clear();
        self.mark(Areas::NOTICES);
    }

    pub fn read_notices_of(&mut self, tab: &str, notify: bool, suggestions: bool) -> bool {
        let mut read = Vec::new();
        for n in &mut self.bell.notices {
            let skip = !suggestions && n.notice.kind == NoticeKind::Suggested;
            if n.notice.tab_id == tab && !n.notice.read && !skip {
                n.notice.read = true;
                read.push(n.id);
            }
        }
        for id in &read {
            self.drop_toast(*id);
        }
        if !read.is_empty() && notify {
            self.mark(Areas::NOTICES);
        }
        !read.is_empty()
    }

    pub fn toggle_notices(&mut self) {
        self.bell.notices_open = !self.bell.notices_open;
        self.bell.sessions_open = false;
        if self.bell.notices_open {
            self.clear_toasts();
        }
        self.mark(Areas::NOTICES);
    }

    pub fn close_notices(&mut self) {
        if std::mem::take(&mut self.bell.notices_open) {
            self.mark(Areas::NOTICES);
        }
    }

    pub fn toggle_sessions(&mut self) {
        self.bell.sessions_open = !self.bell.sessions_open;
        if self.bell.sessions_open {
            self.bell.notices_open = false;
        }
        self.mark(Areas::NOTICES);
    }

    pub fn close_sessions(&mut self) {
        if std::mem::take(&mut self.bell.sessions_open) {
            self.mark(Areas::NOTICES);
        }
    }

    pub fn open_session(&mut self, tab: &str) {
        self.close_sessions();
        self.select(tab);
    }

    pub fn open_notice(&mut self, id: u64) {
        let Some(tab) = self.notice(id).map(|n| n.notice.tab_id.clone()) else { return };
        self.drop_toast(id);
        if !self.has_tab(&tab) {
            if let Some(n) = self.bell.notices.iter_mut().find(|n| n.id == id) {
                n.notice.read = true;
            }
            self.mark(Areas::NOTICES);
            return;
        }
        self.read_notices_of(&tab, true, true);
        self.bell.notices_open = false;
        self.mark(Areas::NOTICES);
        self.select(&tab);
    }

    pub fn dismiss_notice(&mut self, id: u64) {
        self.drop_toast(id);
        self.bell.notices.retain(|n| n.id != id);
        self.mark(Areas::NOTICES);
    }

    pub fn read_all_notices(&mut self) {
        for n in &mut self.bell.notices {
            n.notice.read = true;
        }
        self.clear_toasts();
    }

    pub fn clear_notices(&mut self) {
        self.bell.notices.clear();
        self.clear_toasts();
    }

    pub fn window_active(&self) -> bool {
        self.bell.window_active
    }

    pub fn set_window_active(&mut self, active: bool) {
        if self.bell.window_active == active {
            return;
        }
        self.bell.window_active = active;
        self.mark(Areas::NOTICES);
        if !active {
            self.bell.away_since = Some(self.now());
            for id in self.bell.toasts.clone() {
                self.timers.cancel(&TimerKey::Toast(id));
            }
            return;
        }
        for id in self.bell.toasts.clone() {
            if !self.bell.held.contains(&id) {
                self.arm_toast(id);
            }
        }
        let away = self.bell.away_since.take();
        let landed: Vec<String> = self
            .tabs
            .iter()
            .filter(|t| t.unseen && t.rested_at.zip(away).is_some_and(|(r, a)| r > a))
            .map(|t| t.id.clone())
            .collect();
        if self.prefs.do_not_disturb || !self.bell.toasts.is_empty() {
            return;
        }
        match landed.as_slice() {
            [] => {}
            [one] => {
                let title = self.title_of(one);
                self.show_banner(&format!("{title} terminou enquanto você estava fora"));
            }
            many => self.show_banner(&format!("{} painéis terminaram enquanto você estava fora", many.len())),
        }
    }

    pub fn see_focused(&mut self) -> bool {
        let Some(id) = self.focused.clone() else { return false };
        if !self.watching(&id) {
            return false;
        }
        let unseen = self.tab(&id).is_some_and(|t| t.unseen);
        if self.read_notices_of(&id, true, true) && !unseen {
            return true;
        }
        if !unseen {
            return false;
        }
        if let Some(t) = self.tab_mut(&id) {
            t.unseen = false;
        }
        self.mark_tab(&id);
        true
    }
}
