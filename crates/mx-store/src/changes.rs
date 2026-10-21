use std::ops::BitOr;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Areas(u16);

impl Areas {
    pub const NONE: Areas = Areas(0);
    pub const SIDEBAR: Areas = Areas(1);
    pub const PANES: Areas = Areas(1 << 1);
    pub const NOTICES: Areas = Areas(1 << 2);
    pub const BANNER: Areas = Areas(1 << 3);
    pub const PREFS: Areas = Areas(1 << 4);
    pub const SEARCH: Areas = Areas(1 << 5);
    pub const GROUPS: Areas = Areas(1 << 6);
    pub const LAUNCHERS: Areas = Areas(1 << 7);
    pub const PLUGINS: Areas = Areas(1 << 8);
    pub const ALL: Areas = Areas(u16::MAX);

    pub fn contains(self, other: Areas) -> bool {
        self.0 & other.0 == other.0 && other.0 != 0
    }

    pub fn intersects(self, other: Areas) -> bool {
        self.0 & other.0 != 0
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl BitOr for Areas {
    type Output = Areas;

    fn bitor(self, rhs: Areas) -> Areas {
        Areas(self.0 | rhs.0)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Changes {
    pub areas: Areas,
    pub tabs: Vec<String>,
}

impl Changes {
    pub fn is_empty(&self) -> bool {
        self.areas.is_empty() && self.tabs.is_empty()
    }

    pub(crate) fn mark(&mut self, areas: Areas) {
        self.areas = self.areas | areas;
    }

    pub(crate) fn mark_tab(&mut self, id: &str) {
        self.areas = self.areas | Areas::SIDEBAR;
        if !self.tabs.iter().any(|t| t == id) {
            self.tabs.push(id.to_owned());
        }
    }
}
