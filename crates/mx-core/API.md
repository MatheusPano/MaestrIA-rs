# mx-core — API

Pure crate (std + serde_json/chrono/regex). No tokio, processes or UI. Times are
`text::Time = chrono::DateTime<Local>`; every fn that needs "now" takes it as a param.

## text
- `utf16_len`, `clip_utf16`, `clip_utf16_ellipsis`, `clip_or_ellipsis(s, max)` (Dart `substring(0,max)+'…'`), `utf16_sum`.
- `base36(u64)` lowercase; `last_segment(path)`.
- `iso8601(&Time)` = Dart `toIso8601String` (local, `.mmm` + `uuu` when µs ≠ 0); `parse_iso8601` lenient (none/`Z`/offset); `parse_iso8601_or(raw, fallback)`.
- `short_ago(at, now)`, `short_until(at, now)`.

## paths
- `expand_home(p)` / `expand_home_with(p, home)` — only `~` and `~/…`.
- `state_dir()` = `$MAESTRIA_HOME` (tilde-expanded) else `$HOME/.maestria`; `state_dir_from(override, home)` pure; `config_path(dir)`; `loose_root(home)`.

## json (lenient access helpers)
`Obj`, `Got<T>` (`typed()` → None on wrong type, `lenient()`), `get_str/get_bool/get_num`, `str_of`, `is_true`, `strings`, `objects`, `f64_value`.

## model
- `Folder` (`new`, `loose(home)`, `to_json`, `from_json` → None on missing/wrong-typed field).
- `Workspace` (+ `has/add_root/is_folder_collapsed/set_folder_collapsed`), `new_workspace_id()` (µs + global seq), `workspace::code_workspace_name(path)`.
- `FeatureOrHotfix`, `FeatureOrHotfixKind` (all PT strings: `label/new_label/the/in_this/this_one/of_this/of_the/dissolved/icon`, `other`).
- `PaneGroup` (raw recipe `Obj`s + raw `tree`; `summary`, `where_`, `count`, `tint_index(n)`), `group::summarize`, `group::tint_index_of`.
- `Launcher`, `LauncherIcon` (`by_name` default terminal, `label`). `Tint` (13 names, `by_name`, `label`).
- `WorktreeInfo::short_label`, `worktree::DETACHED`.
- `ClaudeStatus` (`name/by_name/label/at_rest/needs_human/call_to_action`).
- `HookState` (all reducer fields; `settle`, `subtitle(now)`, `plan`, `busy_forks`, `push_suggestion`), `PlanNote` (`headline`, json), `SuggestedTask` (json), caps `MAX_TOUCHED/MAX_PLANS/MAX_SUGGESTIONS`.
- `FollowUp`/`FollowUpKind` (PT strings, `hands_back`), `FlowHold::label`.
- `Notice`, `NoticeKind::verb`, `NOTICE_CAP`, `TOAST_CAP`, `SuggestionChoice`, `SuggestionEnd`.
- `AgentInfo::from_json` (`id` = background job id) / `list_from_stdout` (parses from first `[`), `ContextUsage::from_statusline`.
- `Doc`/`DocSource` (json, 128 KiB UTF-16 text clip), `Setup`, `PluginViewRef`, `is_markdown_path`.
- `Tab` + `TabKind`: pure panel record. Folder/launcher are referenced by `folder_root`+`loose` and `launcher_id`; the PTY state is mirrored in `exited`/`exit_code` by the store. `title(&TabCtx)`, `subtitle(&TabCtx)` (`TabCtx{folder, launcher, doc_missing, now}`), `status`, `resume_id`, `resumable`, `can_hibernate`, `rested`, `rested_ago(now)`, `recipe()` (group recipe) and `to_json()` (layout entry, which alone carries the panel `hotkey` chord id), exact key order.
- `Filter`/`FilterGroup` (`matches(&Tab)`).
- `sidebar::{RowKey, sidebar_rows, root_order_keys, folders_of, workspaces_of, stands_alone}`.
- `ids::{micros_now, run_stamp, tab_id, sidebar_tab_id, feature_id, group_id, launcher_id, suggestion_id(bytes)}`.

## hook
- `apply(&mut HookState, event, &payload, now)` (reads `$HOME` for `~`), `apply_with_home(.., home)` — exact reducer of spec 03 §1.6; non-string `session_id`/`permission_mode` ignored.
- `clip(text, max)` (whitespace-collapse + `…`), `shell_markdown(cmd)`, `absolute(path, cwd, home)`, `EVENTS`.
- Store-side effects (notices, rest/unseen, save) are NOT here (spec 03 §1.7).

## layout
- `PaneNode::{Leaf, Split(PaneSplit)}`, `Axis`, `DropSide`, `MIN_WIDTH/MIN_HEIGHT`.
- Consuming ops: `insert(tab, target, side)`, `remove(tab) -> Option<PaneNode>`, `edge(tab, side)`.
  In place: `swap`, `PaneSplit::resize(gutter, delta, total)`, `split_at_mut(path)`.
  Queries: `order`, `has`, `count`, `same_shape`; free `order(Option<&_>)`, `same_shape(Option, Option)`.
- `to_json(Option<&PaneNode>, &dyn Fn(&str)->i64)`, `from_json(&Value, &dyn Fn(i64)->Option<String>)`, `legacy_two_slot(left, right)`.

## persist
- `Config` — every `config.json` key (raw `Obj` for `floats`, `shortcuts`, `plugins`; `theme: Option<String>` raw;
  `Typography`; `layout: Option<Layout{panes: Vec<Obj>, tree, focused, legacy}>`).
  `Config::from_json(&Obj) -> (Config, Skipped)` applies all legacy migrations and skips bad entries;
  `to_json()` / `to_json_string()` write the exact key order and omit-when-default rules; `rootOrder` is written as the derived sidebar order.
- `read_sidebar(&Obj) -> Sidebar` (folders, features, workspaces incl. 2.4.0 stamps, raw root_order).
- `resolve_theme(raw, &known)` (retired-id map → `maestria`). `Typography::{new, clamp_zoom, size_at}`.
- `PaneEntry::parse(&Obj, now)` (legacy `projectRoot`/`projectId`, `PaneKind` incl. `Browser`=drop, unknown→`Shell`, `asleep()`), `feature_or_hotfix_id_in`.
- `restore_tree(&Layout, &[Option<tab_id>]) -> Option<RestoredLayout{tree: TreeRestore::{Replace(_), KeepPlaced}, focused}>`;
  `None` = nothing restored. With `KeepPlaced` or `focused == None`, the store falls back to its placed tree / first leaf.
- I/O: `load(path) -> LoadOutcome::{Missing, Loaded{config, skipped, backed_up}, Unreadable{error, backed_up}}`
  (`.bak` written on parse failure and when entries were skipped), `write_atomic(path, text, pid)` (`config.json.<pid>.tmp` + rename),
  `save(path, &Config)`, `backup_path`, `temp_path`. Debounce is the store's job.

## ports (object-safe, no runtime)
`BoxFuture`, `CommandRunner::run(command, cwd) -> BoxFuture<CommandOutput>` (`ok`, `spawn_failure`), `Clock::now` (+`SystemClock`),
`Term` (start_shell/start_command/park/remark/kill/hang_up/relaunch/submit/pid/exited/exit_code), `TermFactory::create(tab_id)`,
`Notifier` (alert/badge/open_link/quick_look/reveal/choose_markdown), `PluginBus::emit(event, data, activations, permission)`,
`HookServer` (port/mcp_config_for/settings_for).
