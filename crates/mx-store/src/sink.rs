use std::sync::Arc;

use mx_claude::mcp::{BoxFuture, NewSuggestion, SuggestionQueued, SuggestionSink, Withdrawal};
use tokio::sync::oneshot;

use crate::msg::{StoreMsg, StoreSender};
use crate::store::Store;

pub struct StoreSink {
    sender: StoreSender,
}

impl SuggestionSink for StoreSink {
    fn suggest(&self, suggestion: NewSuggestion) -> BoxFuture<'_, Option<SuggestionQueued>> {
        let (reply, answer) = oneshot::channel();
        self.sender.send(StoreMsg::Suggest(suggestion, reply));
        Box::pin(async move { answer.await.ok().flatten() })
    }

    fn withdraw(&self, tab_id: String, task_id: String) -> BoxFuture<'_, Withdrawal> {
        let (reply, answer) = oneshot::channel();
        self.sender.send(StoreMsg::Withdraw { tab_id, task_id, reply });
        Box::pin(async move {
            answer.await.unwrap_or(Withdrawal { withdrawn: false, already: None })
        })
    }
}

impl Store {
    pub fn sink(&self) -> Arc<dyn SuggestionSink> {
        Arc::new(StoreSink { sender: self.sender() })
    }
}
