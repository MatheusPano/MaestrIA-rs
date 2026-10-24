use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use mx_sys::agents::{AgentInfo, AgentsWatcher, COMMAND, parse_agents};
use mx_sys::{BoxFuture, CommandRunner, FakeRunner, ShResult, output};

const LIST: &str = r#"warning: something
[{"pid": 4242, "cwd": "/tmp", "sessionId": "s1", "name": "api", "status": "busy",
  "waitingFor": "input", "state": "x", "startedAt": 1700000000000.0},
 {"cwd": "/b", "kind": "background"}, 7, "x"]"#;

#[test]
fn parse_starts_at_the_first_bracket_and_keeps_only_objects() {
    let list = parse_agents(LIST).unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(
        list[0],
        AgentInfo {
            pid: Some(4242),
            cwd: "/tmp".into(),
            kind: "interactive".into(),
            session_id: Some("s1".into()),
            name: Some("api".into()),
            status: Some("busy".into()),
            waiting_for: Some("input".into()),
            state: Some("x".into()),
            started_at: Some(1_700_000_000_000),
        }
    );
    assert_eq!(list[1].pid, None);
    assert_eq!(list[1].kind, "background");
}

#[test]
fn parse_rejects_output_without_a_list() {
    assert_eq!(parse_agents("nope"), None);
    assert_eq!(parse_agents("[1,"), None);
    assert_eq!(parse_agents("{\"a\": [1]}"), None);
}

fn runs(fake: &FakeRunner) -> usize {
    fake.commands().iter().filter(|c| c.as_str() == COMMAND).count()
}

async fn settle() {
    for _ in 0..10 {
        tokio::task::yield_now().await;
    }
}

#[tokio::test(start_paused = true)]
async fn start_ticks_now_then_every_two_seconds_while_eager() {
    let fake = Arc::new(FakeRunner::new());
    fake.respond(COMMAND, output(0, LIST, ""));
    let mut w = AgentsWatcher::new(fake.clone());
    let mut rx = w.subscribe();
    w.start();
    settle().await;
    assert_eq!(runs(&fake), 1);
    assert!(rx.has_changed().unwrap());
    assert_eq!(rx.borrow_and_update().len(), 2);
    assert_eq!(w.latest().len(), 2);
    tokio::time::sleep(Duration::from_millis(2001)).await;
    settle().await;
    assert_eq!(runs(&fake), 2);
}

#[tokio::test(start_paused = true)]
async fn idle_polls_every_thirty_seconds_and_turning_eager_ticks_at_once() {
    let fake = Arc::new(FakeRunner::new());
    fake.respond(COMMAND, output(0, "[]", ""));
    let mut w = AgentsWatcher::new(fake.clone());
    w.set_eager(false);
    w.start();
    settle().await;
    assert_eq!(runs(&fake), 1);
    tokio::time::sleep(Duration::from_secs(29)).await;
    settle().await;
    assert_eq!(runs(&fake), 1);
    tokio::time::sleep(Duration::from_millis(1001)).await;
    settle().await;
    assert_eq!(runs(&fake), 2);
    w.set_eager(true);
    settle().await;
    assert_eq!(runs(&fake), 3);
    w.set_eager(true);
    settle().await;
    assert_eq!(runs(&fake), 3);
}

#[tokio::test(start_paused = true)]
async fn a_failed_or_garbled_poll_keeps_the_previous_list() {
    let fake = Arc::new(FakeRunner::new());
    fake.respond_once(COMMAND, output(0, LIST, ""));
    fake.respond_once(COMMAND, output(1, "", "boom"));
    fake.respond_once(COMMAND, output(0, "garbage", ""));
    let mut w = AgentsWatcher::new(fake.clone());
    w.start();
    for _ in 0..2 {
        tokio::time::sleep(Duration::from_millis(2001)).await;
        settle().await;
    }
    assert_eq!(runs(&fake), 3);
    assert_eq!(w.latest().len(), 2);
}

#[tokio::test(start_paused = true)]
async fn stop_halts_polling() {
    let fake = Arc::new(FakeRunner::new());
    let mut w = AgentsWatcher::new(fake.clone());
    w.start();
    settle().await;
    w.stop();
    assert!(!w.is_running());
    tokio::time::sleep(Duration::from_secs(10)).await;
    settle().await;
    assert_eq!(runs(&fake), 1);
}

struct Slow {
    calls: AtomicUsize,
}

impl CommandRunner for Slow {
    fn run(&self, _: String, _: Option<String>) -> BoxFuture<'_, ShResult> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Box::pin(async {
            tokio::time::sleep(Duration::from_secs(5)).await;
            output(0, "[]", "")
        })
    }
}

#[tokio::test(start_paused = true)]
async fn a_tick_is_skipped_while_one_is_in_flight() {
    let slow = Arc::new(Slow { calls: AtomicUsize::new(0) });
    let mut w = AgentsWatcher::new(slow.clone());
    w.start();
    settle().await;
    tokio::time::sleep(Duration::from_millis(4100)).await;
    settle().await;
    assert_eq!(slow.calls.load(Ordering::SeqCst), 1);
    tokio::time::sleep(Duration::from_millis(2000)).await;
    settle().await;
    assert_eq!(slow.calls.load(Ordering::SeqCst), 2);
}

struct Hung {
    calls: AtomicUsize,
}

impl CommandRunner for Hung {
    fn run(&self, _: String, _: Option<String>) -> BoxFuture<'_, ShResult> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Box::pin(std::future::pending())
    }
}

#[tokio::test(start_paused = true)]
async fn a_hung_poll_is_abandoned_after_the_timeout_and_polling_resumes() {
    let hung = Arc::new(Hung { calls: AtomicUsize::new(0) });
    let mut w = AgentsWatcher::new(hung.clone());
    w.start();
    settle().await;
    tokio::time::sleep(mx_sys::agents::TIMEOUT - Duration::from_millis(100)).await;
    settle().await;
    assert_eq!(hung.calls.load(Ordering::SeqCst), 1);
    tokio::time::sleep(Duration::from_millis(2200)).await;
    settle().await;
    assert_eq!(hung.calls.load(Ordering::SeqCst), 2);
}
