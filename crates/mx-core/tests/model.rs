use chrono::{Local, TimeZone};
use mx_core::model::group::summarize;
use mx_core::model::ids;
use mx_core::model::workspace::code_workspace_name;
use mx_core::model::*;
use mx_core::text::Time;
use serde_json::json;

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

#[test]
fn a_shell_in_a_subfolder_without_branch_is_named_by_the_folder() {
    let f = repo();
    let t = Tab::new("t", &f, TabKind::Shell, "/repo/api", now());
    assert_eq!(t.title(&ctx(&f)), "api");
}

#[test]
fn a_tab_at_the_folder_root_is_named_after_the_folder() {
    let f = repo();
    let mut t = Tab::new("t", &f, TabKind::Claude, "/repo", now());
    t.branch = "master".into();
    assert_eq!(t.title(&ctx(&f)), "meu-repo");
}

#[test]
fn folder_round_trip_ignores_legacy_keys() {
    let f = Folder::from_json(&json!({"root": "/repo", "name": "meu-repo", "collapsed": true, "worktreesCollapsed": false})).unwrap();
    assert_eq!(f.name, "meu-repo");
    assert!(f.collapsed);
    let f = Folder::from_json(&json!({"root": "/repo", "name": "meu-repo"})).unwrap();
    assert!(!f.collapsed);
    assert_eq!(
        Folder::from_json(&json!({"root": "/repo", "name": 5})),
        None
    );
    let mut f = repo();
    f.tint = Some(Tint::Cyan);
    assert_eq!(Folder::from_json(&f.to_json()), Some(f));
}

#[test]
fn feature_round_trip_keeps_id_name_brief_and_root() {
    let mut x = FeatureOrHotfix::new(
        "pj1",
        "/repo",
        "permissão do google",
        FeatureOrHotfixKind::Feature,
    );
    x.brief = "contexto".into();
    let back = FeatureOrHotfix::from_json(&x.to_json()).unwrap();
    assert_eq!(back, x);
}

#[test]
fn hotfix_kind_round_trips_and_missing_kind_is_feature() {
    let x = FeatureOrHotfix::new("pj2", "/repo", "login", FeatureOrHotfixKind::Hotfix);
    assert_eq!(x.to_json()["kind"], json!("hotfix"));
    assert_eq!(
        FeatureOrHotfix::from_json(&x.to_json()).unwrap().kind,
        FeatureOrHotfixKind::Hotfix
    );
    let legacy = json!({"id": "pj1", "folderRoot": "/repo", "name": "n"});
    assert_eq!(
        FeatureOrHotfix::from_json(&legacy).unwrap().kind,
        FeatureOrHotfixKind::Feature
    );
    assert_eq!(FeatureOrHotfixKind::Hotfix.dissolved(), "Hotfix dissolvido");
    assert_eq!(
        FeatureOrHotfixKind::Feature.other(),
        FeatureOrHotfixKind::Hotfix
    );
}

#[test]
fn workspace_round_trip_and_omissions() {
    let mut w = Workspace::new("ws1", "atrium");
    w.collapsed = true;
    w.code_workspace_path = Some("/repos/atrium.code-workspace".into());
    w.folder_roots = vec!["/a".into(), "/b".into()];
    w.collapsed_folders = vec!["/b".into()];
    w.tint = Some(Tint::Cyan);
    assert_eq!(Workspace::from_json(&w.to_json()), Some(w));
    let plain = Workspace::new("ws2", "uplii").to_json();
    assert!(plain.get("codeWorkspacePath").is_none());
    assert!(plain.get("collapsed").is_none());
    assert_eq!(Workspace::from_json(&json!({"id": "x"})), None);
    assert_eq!(Workspace::from_json(&json!("nada")), None);
}

#[test]
fn a_2_4_0_workspace_gets_an_id_and_its_path() {
    let w = Workspace::from_json(
        &json!({"path": "/repos/atrium.code-workspace", "name": "atrium", "collapsed": true}),
    )
    .unwrap();
    assert!(!w.id.is_empty());
    assert_eq!(
        w.code_workspace_path.as_deref(),
        Some("/repos/atrium.code-workspace")
    );
    assert!(w.collapsed);
    assert!(w.folder_roots.is_empty());
}

#[test]
fn workspace_ids_are_distinct_within_the_same_instant() {
    let a = new_workspace_id();
    let b = new_workspace_id();
    assert!(a.starts_with("ws"));
    assert_ne!(a, b);
}

#[test]
fn code_workspace_name_strips_the_extension_case_insensitively() {
    assert_eq!(code_workspace_name("/repos/cefis.code-workspace"), "cefis");
    assert_eq!(code_workspace_name("/repos/Cefis.Code-Workspace"), "Cefis");
    assert_eq!(
        code_workspace_name("/repos/.code-workspace"),
        ".code-workspace"
    );
}

#[test]
fn invalid_groups_read_as_nothing() {
    for j in [
        json!({"id": "gp1", "name": "grid", "panes": []}),
        json!({"name": "  ", "panes": [{}]}),
        json!({"name": "grid", "panes": [{}]}),
        json!("grid"),
        json!({"id": "gp1", "name": 5, "panes": [{}]}),
    ] {
        assert_eq!(PaneGroup::from_json(&j), None, "{j}");
    }
}

#[test]
fn group_summary_where_and_round_trip() {
    let panes: Vec<_> = [
        json!({"folderRoot": "/repo", "kind": "shell", "cwd": "/repo"}),
        json!({"folderRoot": "/repo", "kind": "shell", "cwd": "/repo"}),
        json!({"folderRoot": "/repo", "kind": "shell", "cwd": "/repo"}),
    ]
    .into_iter()
    .map(|v| v.as_object().unwrap().clone())
    .collect();
    let g = PaneGroup {
        id: "gp1".into(),
        name: "grid de 3".into(),
        panes,
        tree: Some(
            json!({"axis": "row", "weights": [0.6, 0.4], "children": [0, {"axis": "column", "weights": [0.5, 0.5], "children": [1, 2]}]}),
        ),
    };
    assert_eq!(g.summary(), "3 terminais");
    assert_eq!(g.where_(), "repo");
    assert_eq!(g.count(), 3);
    let back = PaneGroup::from_json(&g.to_json()).unwrap();
    assert_eq!(back, g);
    assert_eq!(back.tint_index(4), g.tint_index(4));
    let single = PaneGroup { tree: None, ..g };
    assert!(single.to_json().get("tree").is_none());
}

#[test]
fn summary_joins_kinds_in_portuguese() {
    let objs = |kinds: &[&str]| -> Vec<_> {
        kinds
            .iter()
            .map(|k| json!({"kind": k}).as_object().unwrap().clone())
            .collect()
    };
    assert_eq!(summarize(&objs(&[])), "");
    assert_eq!(summarize(&objs(&["claude"])), "uma sessão");
    assert_eq!(
        summarize(&objs(&["claude", "shell", "reader", "reader"])),
        "uma sessão, um terminal e 2 leitores"
    );
    assert_eq!(
        summarize(&objs(&["shell", "claude", "claude"])),
        "2 sessões e um terminal"
    );
    let loose: Vec<_> = vec![
        json!({"loose": true, "folderRoot": "/home/u"})
            .as_object()
            .unwrap()
            .clone(),
    ];
    let g = PaneGroup {
        id: "x".into(),
        name: "n".into(),
        panes: loose,
        tree: None,
    };
    assert_eq!(g.where_(), "avulsos");
}

#[test]
fn launchers_validate_and_default_the_icon() {
    let l = Launcher::from_json(
        &json!({"id": "lch1", "name": " btop ", "command": " btop ", "icon": "rocketship"}),
    )
    .unwrap();
    assert_eq!(l.name, "btop");
    assert_eq!(l.icon, LauncherIcon::Terminal);
    assert_eq!(
        Launcher::from_json(&json!({"id": "lch1", "name": "x", "command": " "})),
        None
    );
    assert_eq!(LauncherIcon::by_name(Some("database")).label(), "Banco");
    assert_eq!(Launcher::from_json(&l.to_json()), Some(l));
}

#[test]
fn unknown_tint_names_read_as_no_tint() {
    assert_eq!(Tint::by_name(Some("blue")), None);
    assert_eq!(Tint::by_name(Some("petrol")), Some(Tint::Petrol));
    assert_eq!(Tint::Aqua.label(), "Verde-água");
}

#[test]
fn worktree_short_label_prefers_the_task_id() {
    let w = |path: &str, branch: &str| WorktreeInfo {
        path: path.into(),
        branch: branch.into(),
        is_main: false,
        prunable: false,
    };
    assert_eq!(w("/x", "feature/TASK#47730").short_label(), "TASK#47730");
    assert_eq!(w("/x", "feature/login").short_label(), "login");
    assert_eq!(w("/repo/wt", "feature/").short_label(), "wt");
}

#[test]
fn status_table_matches_the_spec() {
    use ClaudeStatus::*;
    let rest: Vec<_> = ClaudeStatus::ALL
        .into_iter()
        .filter(|s| s.at_rest())
        .collect();
    assert_eq!(rest, [WaitingInput, Idle]);
    let human: Vec<_> = ClaudeStatus::ALL
        .into_iter()
        .filter(|s| s.needs_human())
        .collect();
    assert_eq!(human, [WaitingInput, WaitingAnswer, WaitingPermission]);
    assert_eq!(Unknown.label(), "—");
    assert_eq!(WaitingInput.label(), "Esperando você");
    assert_eq!(Idle.call_to_action(), "te espera");
    assert_eq!(
        ClaudeStatus::by_name("waitingPermission"),
        Some(WaitingPermission)
    );
    assert_eq!(HookState::new().status, Starting);
}

#[test]
fn waiting_permission_without_tool_prints_null_like_dart() {
    let s = HookState {
        status: ClaudeStatus::WaitingPermission,
        ..HookState::new()
    };
    assert_eq!(s.subtitle(&now()), "Quer aprovação pra null");
}

#[test]
fn plan_headline_skips_heading_marks_and_clips_at_70() {
    let p = |t: &str| PlanNote {
        text: t.into(),
        at: now(),
    };
    assert_eq!(p("\n## Migrar o banco\nresto").headline(), "Migrar o banco");
    assert_eq!(p("   \n#\n").headline(), "Plano sem título");
    let long = "x".repeat(80);
    assert_eq!(p(&long).headline(), format!("{}…", "x".repeat(70)));
}

#[test]
fn plan_and_suggestion_json_are_lenient() {
    let plan = PlanNote::from_json(
        &json!({"text": "# p", "at": "2026-09-18T12:00:00.000"}),
        now(),
    )
    .unwrap();
    assert_eq!(plan.at, now());
    assert_eq!(PlanNote::from_json(&json!({"text": "  "}), now()), None);
    assert_eq!(PlanNote::from_json(&json!("x"), now()), None);
    let fallback = PlanNote::from_json(&json!({"text": "p", "at": "ontem"}), now()).unwrap();
    assert_eq!(fallback.at, now());
    assert_eq!(
        plan.to_json(),
        json!({"text": "# p", "at": "2026-09-18T12:00:00.000"})
    );

    let t =
        SuggestedTask::from_json(&json!({"id": "task_0a1bc2d3", "prompt": "faz"}), now()).unwrap();
    assert_eq!(t.title, "");
    let keys: Vec<String> = t.to_json().as_object().unwrap().keys().cloned().collect();
    assert_eq!(keys, ["id", "title", "tldr", "prompt", "at"]);
    assert_eq!(
        SuggestedTask::from_json(&json!({"prompt": "faz"}), now()),
        None
    );
    assert_eq!(
        SuggestedTask::from_json(&json!({"id": "t", "prompt": " "}), now()),
        None
    );
}

#[test]
fn follow_ups_read_known_kinds_only() {
    let f =
        FollowUp::from_json(&json!({"kind": "handoff", "text": "assume", "target": "t2"})).unwrap();
    assert_eq!(f.target_tab_id.as_deref(), Some("t2"));
    assert_eq!(FollowUp::from_json(&f.to_json()), Some(f));
    assert_eq!(FollowUp::from_json(&json!({"kind": "nope"})), None);
    assert_eq!(
        FollowUp::from_json(&json!({"kind": "command"}))
            .unwrap()
            .text,
        ""
    );
    assert!(FollowUpKind::KeepGoing.hands_back());
    assert!(!FollowUpKind::Command.hands_back());
    assert_eq!(
        FlowHold::Forks.label(),
        "esperando os agentes que ela abriu"
    );
}

#[test]
fn context_usage_reads_the_statusline_payload() {
    let j = json!({"context_window": {"used_percentage": 41.6, "context_window_size": 200000,
        "current_usage": {"input_tokens": 10, "cache_creation_input_tokens": 20.4, "cache_read_input_tokens": "x"}}});
    assert_eq!(
        ContextUsage::from_statusline(&j),
        Some(ContextUsage {
            percent: 42,
            tokens: 30,
            window: 200000
        })
    );
    let zero = json!({"context_window": {"used_percentage": 1, "context_window_size": 0, "current_usage": {}}});
    assert_eq!(ContextUsage::from_statusline(&zero), None);
    assert_eq!(ContextUsage::from_statusline(&json!({})), None);
}

#[test]
fn agents_parse_from_the_first_bracket() {
    let rows = AgentInfo::list_from_stdout(
        "aviso\n[{\"pid\": 4242, \"sessionId\": \"s1\", \"startedAt\": 1.9e3}, 3]",
    )
    .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].pid, Some(4242));
    assert_eq!(rows[0].kind, "interactive");
    assert_eq!(rows[0].cwd, "");
    assert_eq!(rows[0].started_at, Some(1900));
    assert_eq!(AgentInfo::list_from_stdout("nada"), None);
}

#[test]
fn identifier_formats() {
    assert_eq!(ids::tab_id("m3x9k2a1b", 0), "tabm3x9k2a1b_0");
    assert_eq!(
        ids::launcher_id(1_729_500_000_000_000),
        "lch1729500000000000"
    );
    assert_eq!(
        ids::suggestion_id([0x0a, 0x1b, 0xc2, 0xd3]),
        "task_0a1bc2d3"
    );
    assert!(ids::feature_id(36).starts_with("pj10"));
    assert_eq!(ids::group_id(35), "gpz");
    assert_eq!(ids::sidebar_tab_id("x"), "sidebar:x");
}

#[test]
fn markdown_paths_are_recognised_by_extension() {
    assert!(is_markdown_path("/a/B.MD"));
    assert!(is_markdown_path("x.mdx"));
    assert!(is_markdown_path("notas.markdown"));
    assert!(!is_markdown_path("/a.md/x.txt"));
}
