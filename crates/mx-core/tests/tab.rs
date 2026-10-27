use chrono::{Local, TimeZone};
use mx_core::model::*;
use mx_core::text::Time;
use serde_json::{Value, json};

fn now() -> Time {
    Local.with_ymd_and_hms(2026, 9, 18, 12, 0, 0).unwrap()
}

fn repo() -> Folder {
    Folder::new("/repo", "meu-repo")
}

fn ctx(folder: &Folder) -> TabCtx<'_> {
    TabCtx {
        folder,
        launcher: None,
        doc_missing: false,
        now: now(),
    }
}

fn worktree_tab(label: Option<&str>) -> Tab {
    let mut t = Tab::new(
        "t1",
        &repo(),
        TabKind::Claude,
        "/repo/.claude/worktrees/TASK-47730",
        now(),
    );
    t.branch = "feature/TASK#47730".into();
    t.custom_label = label.map(str::to_owned);
    t
}

#[test]
fn a_custom_label_wins_the_title() {
    let f = repo();
    assert_eq!(
        worktree_tab(Some("refatorar auth")).title(&ctx(&f)),
        "refatorar auth"
    );
}

#[test]
fn without_label_the_title_is_the_task_in_the_branch() {
    let f = repo();
    assert_eq!(worktree_tab(None).title(&ctx(&f)), "TASK#47730");
}

#[test]
fn a_claude_tab_with_label_and_session_serializes_exactly() {
    let mut t = worktree_tab(Some("TASK#47730"));
    t.session_id = Some("abc-123".into());
    assert_eq!(
        serde_json::to_string(&t.to_json()).unwrap(),
        r#"{"folderRoot":"/repo","kind":"claude","cwd":"/repo/.claude/worktrees/TASK-47730","label":"TASK#47730","sessionId":"abc-123"}"#
    );
}

#[test]
fn a_shell_never_writes_its_session_id() {
    let mut t = Tab::new("t", &repo(), TabKind::Shell, "/repo", now());
    t.session_id = Some("abc".into());
    assert!(t.to_json().get("sessionId").is_none());
}

#[test]
fn hibernated_and_pinned_and_done_live_in_the_layout_not_the_recipe() {
    let mut t = Tab::new("t1", &repo(), TabKind::Claude, "/repo", now());
    t.session_id = Some("sess-1".into());
    assert!(t.to_json().get("done").is_none());
    t.hibernated = true;
    t.pinned = true;
    t.done = true;
    t.group_id = Some("gp1".into());
    t.hooks.touched.push("/repo/a.md".into());
    let j = t.to_json();
    assert_eq!(j["hibernated"], json!(true));
    assert_eq!(j["pinned"], json!(true));
    assert_eq!(j["done"], json!(true));
    assert_eq!(j["sessionId"], json!("sess-1"));
    let recipe = t.recipe();
    for k in ["hibernated", "pinned", "done", "group", "touched"] {
        assert!(!recipe.contains_key(k), "{k}");
    }
    let keys: Vec<&String> = j.as_object().unwrap().keys().collect();
    assert_eq!(
        keys,
        [
            "folderRoot",
            "kind",
            "cwd",
            "sessionId",
            "group",
            "touched",
            "done",
            "hibernated",
            "pinned"
        ]
    );
}

#[test]
fn a_hibernated_tab_says_so_in_the_subtitle() {
    let f = repo();
    let mut t = Tab::new("t1", &f, TabKind::Claude, "/repo", now());
    t.session_id = Some("sess-1".into());
    assert!(t.can_hibernate());
    t.hibernated = true;
    t.exited = true;
    assert!(t.subtitle(&ctx(&f)).contains("Hibernada"));
    assert!(!t.can_hibernate());
    assert!(t.resumable());
}

#[test]
fn loose_recipe_carries_the_loose_flag() {
    let loose = Folder::loose(Some("/home/u"));
    let t = Tab::new("t", &loose, TabKind::Shell, "/home/u", now());
    assert_eq!(t.recipe()["loose"], json!(true));
    assert_eq!(t.title(&ctx(&loose)), "~");
    assert_eq!(loose.name, "Avulsos");
}

#[test]
fn subtitles_follow_kind_and_process_state() {
    let f = repo();
    let mut t = Tab::new("t", &f, TabKind::Shell, "/repo", now());
    assert_eq!(t.subtitle(&ctx(&f)), "Shell");
    t.exited = true;
    assert_eq!(t.subtitle(&ctx(&f)), "Processo saiu (?)");
    t.exit_code = Some(2);
    assert_eq!(t.subtitle(&ctx(&f)), "Processo saiu (2)");
    assert_eq!(t.status(), ClaudeStatus::Ended);
    let l = Launcher {
        id: "lch1".into(),
        name: "btop".into(),
        command: "btop -p".into(),
        icon: LauncherIcon::Monitor,
    };
    let mut p = Tab::new("t", &f, TabKind::Shell, "/repo", now());
    p.launcher_id = Some(l.id.clone());
    let c = TabCtx {
        folder: &f,
        launcher: Some(&l),
        doc_missing: false,
        now: now(),
    };
    assert_eq!(p.title(&c), "btop");
    assert_eq!(p.subtitle(&c), "btop -p");
    assert_eq!(p.recipe()["launcher"], json!("lch1"));
}

#[test]
fn reader_subtitle_names_source_origin_and_time() {
    let f = repo();
    let mut t = Tab::new("t", &f, TabKind::Reader, "/repo", now());
    let mut doc = Doc::file("/repo/PLANO.md", Some("TASK#1".into()), now());
    doc.at = Local.with_ymd_and_hms(2026, 9, 18, 9, 5, 0).unwrap();
    t.doc = Some(doc);
    assert_eq!(t.title(&ctx(&f)), "PLANO.md");
    assert_eq!(t.subtitle(&ctx(&f)), "arquivo · de TASK#1 · 09:05");
    let missing = TabCtx {
        doc_missing: true,
        ..ctx(&f)
    };
    assert_eq!(t.subtitle(&missing), "arquivo · o arquivo não está mais lá");
    let j = t.to_json();
    assert!(j["doc"].get("text").is_none());
    assert_eq!(j["doc"]["path"], json!("/repo/PLANO.md"));
}

#[test]
fn filters_match_by_kind_and_state() {
    let f = repo();
    let mut waiting = Tab::new("w", &f, TabKind::Claude, "/repo", now());
    waiting.hooks.status = ClaudeStatus::WaitingPermission;
    let mut working = Tab::new("k", &f, TabKind::Claude, "/repo", now());
    working.hooks.status = ClaudeStatus::Working;
    let shell = Tab::new("s", &f, TabKind::Shell, "/repo", now());
    assert!(Filter::Waiting.matches(&waiting) && !Filter::Waiting.matches(&working));
    assert!(Filter::Working.matches(&working));
    assert!(Filter::Shell.matches(&shell) && !Filter::Claude.matches(&shell));
    waiting.done = true;
    assert!(!Filter::Waiting.matches(&waiting));
    assert!(Filter::Finished.matches(&waiting));
    let mut exited = shell.clone();
    exited.exited = true;
    assert!(Filter::Parked.matches(&exited));
    assert_eq!(Filter::Parked.group().label(), "Estado");
}

#[test]
fn rested_needs_rest_status_and_a_live_process() {
    let f = repo();
    let mut t = Tab::new("t", &f, TabKind::Claude, "/repo", now());
    t.hooks.status = ClaudeStatus::Idle;
    t.rested_at = Some(now());
    assert!(t.rested());
    assert_eq!(t.rested_ago(&now()).as_deref(), Some("agora"));
    t.exited = true;
    assert!(!t.rested());
    assert_eq!(t.rested_ago(&now()), None);
}

#[test]
fn docs_without_a_path_for_files_are_dropped() {
    assert_eq!(Doc::from_json(&json!({"source": "file"}), now()), None);
    assert_eq!(Doc::from_json(&json!({}), now()), None);
    let plan = Doc::from_json(&json!({"source": "plan", "text": "# x"}), now()).unwrap();
    assert_eq!(plan.title, "plano");
    let big = Doc {
        text: "a".repeat(131_073),
        ..plan
    };
    let saved = big.to_json()["text"].as_str().unwrap().to_owned();
    assert!(saved.ends_with("\n\n…"));
    assert_eq!(saved.len(), 131_072 + "\n\n…".len());
    assert_eq!(Setup::from_json(&json!({"root": "  "})), None);
    let s = Setup::from_json(&json!({"root": "/repo", "selected": "CLAUDE.md"})).unwrap();
    assert_eq!(
        s.to_json(),
        json!({"root": "/repo", "selected": "CLAUDE.md"})
    );
    let _: Value = s.to_json();
}
