#![allow(dead_code)]

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use mx_claude::mcp::{
    BoxFuture, NewSuggestion, SuggestionEnd, SuggestionQueued, SuggestionSink, TaskRef, Withdrawal,
};
use mx_claude::{HookServer, ServerEvent};
use serde_json::Value;
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};

pub async fn start(state: &Path) -> (HookServer, UnboundedReceiver<ServerEvent>) {
    let (tx, rx) = unbounded_channel();
    let server = HookServer::start(state, tx).await.expect("server starts");
    (server, rx)
}

pub async fn post(port: u16, path: &str, body: &str) -> (u16, String) {
    let res = reqwest::Client::new()
        .post(format!("http://127.0.0.1:{port}{path}"))
        .header("content-type", "application/json")
        .body(body.to_owned())
        .send()
        .await
        .expect("request");
    let status = res.status().as_u16();
    (status, res.text().await.expect("body"))
}

pub fn stop_url_port(settings: &str) -> u16 {
    let json: Value = serde_json::from_str(settings).unwrap();
    let url = json["hooks"]["Stop"][0]["hooks"][0]["url"]
        .as_str()
        .unwrap();
    let rest = url.strip_prefix("http://127.0.0.1:").unwrap();
    rest.split('/').next().unwrap().parse().unwrap()
}

#[derive(Default)]
pub struct DeskState {
    pub pending: Vec<(TaskRef, String)>,
    pub ended: HashMap<String, SuggestionEnd>,
    pub seq: usize,
}

#[derive(Default)]
pub struct Desk(pub Mutex<DeskState>);

impl Desk {
    pub fn pending(&self) -> Vec<(TaskRef, String)> {
        self.0.lock().unwrap().pending.clone()
    }
}

impl SuggestionSink for Desk {
    fn suggest(&self, s: NewSuggestion) -> BoxFuture<'_, Option<SuggestionQueued>> {
        Box::pin(async move {
            if s.tab_id != "tab1" {
                return None;
            }
            let mut state = self.0.lock().unwrap();
            let task = TaskRef {
                id: format!("task_0000000{}", state.seq),
                title: s.title,
            };
            state.seq += 1;
            state.pending.push((task.clone(), s.prompt));
            Some(SuggestionQueued {
                task,
                pending: state.pending.iter().map(|(t, _)| t.clone()).collect(),
                evicted: Vec::new(),
            })
        })
    }

    fn withdraw(&self, _tab_id: String, task_id: String) -> BoxFuture<'_, Withdrawal> {
        Box::pin(async move {
            let mut state = self.0.lock().unwrap();
            let before = state.pending.len();
            state.pending.retain(|(t, _)| t.id != task_id);
            if state.pending.len() < before {
                state.ended.insert(task_id, SuggestionEnd::Dismissed);
                return Withdrawal {
                    withdrawn: true,
                    already: None,
                };
            }
            Withdrawal {
                withdrawn: false,
                already: state.ended.get(&task_id).copied(),
            }
        })
    }
}
