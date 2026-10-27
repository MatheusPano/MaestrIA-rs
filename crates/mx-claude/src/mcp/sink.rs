use std::future::Future;
use std::pin::Pin;
use std::sync::OnceLock;

use rand::RngCore;
use regex::Regex;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub const MAX_PENDING: usize = 20;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskRef {
    pub id: String,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewSuggestion {
    pub tab_id: String,
    pub title: String,
    pub tldr: String,
    pub prompt: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuggestionQueued {
    pub task: TaskRef,
    pub pending: Vec<TaskRef>,
    pub evicted: Vec<TaskRef>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SuggestionEnd {
    Started,
    Dismissed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Withdrawal {
    pub withdrawn: bool,
    pub already: Option<SuggestionEnd>,
}

pub trait SuggestionSink: Send + Sync {
    fn suggest(&self, suggestion: NewSuggestion) -> BoxFuture<'_, Option<SuggestionQueued>>;
    fn withdraw(&self, tab_id: String, task_id: String) -> BoxFuture<'_, Withdrawal>;
}

pub struct NoSink;

impl SuggestionSink for NoSink {
    fn suggest(&self, _: NewSuggestion) -> BoxFuture<'_, Option<SuggestionQueued>> {
        Box::pin(async { None })
    }

    fn withdraw(&self, _: String, _: String) -> BoxFuture<'_, Withdrawal> {
        Box::pin(async {
            Withdrawal {
                withdrawn: false,
                already: None,
            }
        })
    }
}

pub fn new_task_id() -> String {
    let mut bytes = [0u8; 4];
    rand::rng().fill_bytes(&mut bytes);
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!("task_{hex}")
}

pub fn is_task_id(id: &str) -> bool {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^task_[0-9a-f]{8}$").unwrap())
        .is_match(id)
}
