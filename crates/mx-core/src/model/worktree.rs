use std::sync::LazyLock;

use regex::Regex;

use crate::text::last_segment;

static TASK_LABEL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"([A-Z]+[#-]?(?-u:\d)+)").expect("valid regex"));

pub const DETACHED: &str = "(detached)";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeInfo {
    pub path: String,
    pub branch: String,
    pub is_main: bool,
    pub prunable: bool,
}

impl WorktreeInfo {
    pub fn short_label(&self) -> String {
        if let Some(m) = TASK_LABEL.find(&self.branch) {
            return m.as_str().to_owned();
        }
        let seg = last_segment(&self.branch);
        if !seg.is_empty() {
            return seg.to_owned();
        }
        last_segment(&self.path).to_owned()
    }
}
