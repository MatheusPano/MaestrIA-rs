use mx_core::text::{clip_utf16, utf16_len};
use serde_json::{Map, Value, json};

use super::sink::{NewSuggestion, SuggestionEnd, SuggestionSink, TaskRef, is_task_id};
use super::{DISMISS_TOOL, MAX_PENDING, MAX_PROMPT, SUGGEST_TOOL};

pub const INSTRUCTIONS: &str = "This session runs inside Maestria, a desktop cockpit for Claude Code sessions. When you notice work that deserves doing but is outside the scope of the current task, call suggest_task instead of widening the task or only mentioning it in prose: the user gets a card they can start as a separate session with one click.";

const SUGGEST_DESCRIPTION: &str = "Suggest a separate task for something you noticed that is outside the scope of your current work.\n\nCall this when you come across something that deserves fixing but does not belong in this change: dead code, stale docs, a missing test, a confirmed TODO, a bug or security issue spotted in passing. Do not use it for trivial fixes you can make inline, for anything the user asked you to do, for vague impressions or unverified hunches, or to split off parts of your own task. The call only queues a suggestion and returns; carry on with your current work.\n\nThe user sees a \"tarefa sugerida\" card in the Maestria panel of this session, with your title and tldr, can read the full prompt, and with one click starts it as a new Claude Code session (in a fresh git worktree, or in their checkout of this repository), sends it to this session, or dismisses it. A new session starts from your prompt alone, on a checkout that may not have this session's changes, so the prompt has to stand alone: what to change and why, the files involved by repository-relative path, and any context from this conversation it depends on. If the find involves a secret or credential, say where it is rather than copying the value, since the prompt is stored and displayed.\n\nThe result carries a task_id; if the suggestion later becomes moot, withdraw it with dismiss_task.";

const DISMISS_DESCRIPTION: &str = "Withdraw a task suggestion you queued earlier with suggest_task.\n\nCall this when a suggestion has gone stale: the issue was fixed in this session (by you or the user), it turned out not to be a problem, or you have queued a better-scoped replacement (queue the replacement first, then dismiss the old task_id).\n\nOnly a suggestion the user has not acted on can be withdrawn. If they already started or dismissed it, the result says so and nothing changes; that answer is final, so do not retry or re-flag it.";

pub fn tools_json() -> Value {
    json!([
        {
            "name": SUGGEST_TOOL,
            "title": "Suggest a task",
            "description": SUGGEST_DESCRIPTION,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "title": {
                        "type": "string",
                        "description": "The card heading and the new session's name: a short imperative phrase starting with a verb, under 60 characters, that makes sense on its own, e.g. \"Fix stale README badge\"."
                    },
                    "tldr": {
                        "type": "string",
                        "description": "One or two plain sentences on what the task would do and why it is worth doing. Shown on the card under the title, so no file paths or code."
                    },
                    "prompt": {
                        "type": "string",
                        "description": format!("The opening message of the new session: the goal, the files involved by repository-relative path, the context it needs from this conversation, and what done looks like. Markdown is fine. Hard limit {MAX_PROMPT} characters.")
                    }
                },
                "required": ["title", "tldr", "prompt"]
            }
        },
        {
            "name": DISMISS_TOOL,
            "title": "Withdraw a suggested task",
            "description": DISMISS_DESCRIPTION,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task_id": {
                        "type": "string",
                        "description": "The task_id from the suggest_task result (task_1a2b3c4d)."
                    },
                    "reason": {
                        "type": "string",
                        "description": "Optional short note on why the suggestion is no longer needed. It does not change the outcome."
                    }
                },
                "required": ["task_id"]
            }
        }
    ])
}

fn text_arg(args: &Map<String, Value>, key: &str) -> String {
    args.get(key)
        .and_then(Value::as_str)
        .map(|s| s.trim().to_owned())
        .unwrap_or_default()
}

fn tool_text(text: String) -> Value {
    json!({"content": [{"type": "text", "text": text}]})
}

pub fn tool_error(text: String) -> Value {
    json!({"content": [{"type": "text", "text": text}], "isError": true})
}

fn list(tasks: &[TaskRef]) -> String {
    if tasks.is_empty() {
        return "none".to_owned();
    }
    tasks
        .iter()
        .map(|t| {
            if t.title.is_empty() {
                t.id.clone()
            } else {
                format!("{} \"{}\"", t.id, t.title)
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

pub(super) async fn suggest(
    args: &Map<String, Value>,
    tab_id: &str,
    sink: &dyn SuggestionSink,
) -> Value {
    let prompt = text_arg(args, "prompt");
    if prompt.is_empty() {
        return tool_error("prompt is required and cannot be empty.".into());
    }
    let len = utf16_len(&prompt);
    if len > MAX_PROMPT {
        return tool_error(format!(
            "prompt is too long ({len} chars; max {MAX_PROMPT}). Trim it to the essentials; the new session can read the files itself."
        ));
    }
    let title = clip_utf16(&text_arg(args, "title"), 200).to_owned();
    let tldr = clip_utf16(&text_arg(args, "tldr"), 2000).to_owned();
    let queued = sink
        .suggest(NewSuggestion {
            tab_id: tab_id.to_owned(),
            title,
            tldr,
            prompt,
        })
        .await;
    let Some(queued) = queued else {
        return tool_error("This session is no longer attached to a Maestria panel, so there is nowhere to show the suggestion. Mention it in your reply instead.".into());
    };
    let dropped = match queued.evicted.len() {
        0 => String::new(),
        n => format!(
            "Pending suggestions are capped at {MAX_PENDING}, so the oldest {} dropped: {}. ",
            if n == 1 {
                "one was".to_owned()
            } else {
                format!("{n} were")
            },
            list(&queued.evicted)
        ),
    };
    tool_text(format!(
        "Noted ({}). A card is showing in the Maestria panel for this session: the user can start it in a new session, in a fresh worktree or in their checkout, send it to this session, or dismiss it. If it becomes stale or superseded, call {DISMISS_TOOL} with this task_id. {dropped}Currently pending: {}. Continue your current work.",
        queued.task.id,
        list(&queued.pending)
    ))
}

pub(super) async fn dismiss(
    args: &Map<String, Value>,
    tab_id: &str,
    sink: &dyn SuggestionSink,
) -> Value {
    let task_id = text_arg(args, "task_id");
    if !is_task_id(&task_id) {
        return tool_error(format!(
            "task_id must be the id returned by {SUGGEST_TOOL} (format: task_xxxxxxxx)."
        ));
    }
    let out = sink.withdraw(tab_id.to_owned(), task_id.clone()).await;
    if out.withdrawn {
        return tool_text(format!(
            "Task {task_id} withdrawn. The card is no longer shown. Continue your current work."
        ));
    }
    tool_text(match out.already {
        Some(SuggestionEnd::Started) => format!(
            "Task {task_id} was already started by the user, so it is no longer pending and cannot be withdrawn. Nothing was changed."
        ),
        Some(SuggestionEnd::Dismissed) => {
            format!("Task {task_id} was already dismissed. Nothing was changed.")
        }
        None => format!(
            "No pending task with id {task_id}: it was never queued from this panel, or the panel was closed. Do not re-flag it. Nothing was changed."
        ),
    })
}
