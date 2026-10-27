# mx-store — API

The AppStore (spec 01 §3, spec 03 §1.7/§2.5, spec 04 §2.4/§4). One owner, `&mut self` on the UI thread.

## Construction and driving
- `Store::new(StoreOptions, Ports)`; `StoreOptions::from_env()` (`MAESTRIA_HOME`, `~/.claude/projects`) or
  `StoreOptions::at(state_dir, chat_home, home)`. `Ports { clock, terms, runner, notifier, plugins, hooks, rt }`
  (mx-core port traits + a tokio `Handle`; pass the `mx_plugins::Plugins` handle as `plugins`).
- Startup order: `new` → `load()` (config.json + layout restore, `.bak` on parse failure) →
  `attach_plugins(Plugins)` → `start()` (ticks 1/10/20 s, agents poll right away).
- Timers: `next_deadline()` → schedule a wake-up at that instant, then `fire_due()`. All timers (save 400 ms,
  settle 3 s, banner 6 s, toast 8 s, reveal 2 s, ticks, agents 2 s eager / 30 s idle) live there.
- Background work runs on `rt` and comes back as messages: `set_waker(Arc<Fn>)` is called whenever one
  arrives; then call `pump()` on the UI thread. `sender() -> StoreSender` lets other threads post
  `StoreMsg::{Server(ServerEvent), TermExited{tab_id, code}, Suggest, Withdraw, Run(FnOnce(&mut Store))}`.
  Tests use `settle().await` (drains until no task is pending).
- Changes: `take_changes() -> Changes { areas: Areas, tabs }`. `Areas::{SIDEBAR, PANES, NOTICES, BANNER,
  PREFS, SEARCH, GROUPS, LAUNCHERS, PLUGINS}`; `tabs` = panels whose row/header changed. Idle ticks mark
  nothing; the 1 s tick marks only tabs running a tool or armed, the 20 s tick only changed age labels.
- `shutdown() -> Future` (flush pending write, then kill every term), `dispose()`, `flush()`, `save()`, `touch()`.

## Ports the app must honour (gaps in mx-core's `Term`)
- The term layer reports process exit with `StoreMsg::TermExited` (ignored if `term.exited()` is false again).
- `start_command` on an exited term must restart it (used by `wake`).
- `Effect::PasteClipboard{tab_id, images_via_ctrl_v}` = a plugin's ⌘V: read the clipboard, then
  `TerminalSession::paste_clipboard`. Terms implement `mx_core::ports::TermInput` (trust prompt, keys, Enter).
- Bracketed paste without Enter is emitted as `Effect::Paste{tab_id, text}`; clipboard as `Effect::Clipboard`.
  Drain with `take_effects()`.

## State (read)
`tabs() tab(id) term(id) folders() loose() workspaces() features() groups() launchers() prefs() search()
bell() banner() notices() toasts() unread_notices() panes() order() focused_pane_id() focused_tab()
open_panes() is_open is_pinned watching title(t) subtitle(t) sidebar_rows() folders_of workspaces_of
stands_alone tabs_of needing_human needing_human_in waiting_sessions worktrees_of agents() config()
revealed() take_reveal_scroll(tab, within) active_group() group_of showing tint_of → TabTint::{Chosen(Tint),
Group(idx of 4)} chosen_tint_of reading() last_import() matches(t) hits() has_hits filtering() active_filters()`.

## Commands (spec 01 §3, same names in snake_case)
- Prefs: `set_theme set_typography zoom_focused reset_zoom_focused set_sidebar_width toggle_sidebar
  set_sidebar_hidden set_status_bar_top set_do_not_disturb toggle_do_not_disturb set_suggestions_to_plugin
  set_hibernate_minutes toggle_groups_collapsed set_sidebar_view set_rail_order set_shortcuts set_floats`.
- Folders/workspaces: `add_folder` (async) `import_workspace` (async, result in `last_import`) `set_folder_tint
  rename_folder toggle_collapsed remove_folder create_workspace -> Result<id, EmptyName> add_to_workspace
  remove_from_workspace rename_workspace set_workspace_tint link_code_workspace dissolve_workspace
  close_workspace closing_with toggle_workspace_collapsed is_folder_collapsed toggle_folder_collapsed
  move_root_row move_in_workspace move_folder can_drop/drop_row(RowPlace, RowPlace, into)`.
- Features: `add_feature edit_feature_text set_feature_tint set_feature_kind toggle_feature_collapsed
  complete_feature remove_feature assign tabs_in`.
- Tabs: `open_shell open_shell_with open_launcher open_claude(ClaudeSpec) open_claude_placed(spec, place) relaunch select place dismiss
  close_tab -> Option<Tab> close_focused close_settled settled_in rename_tab set_tab_tint set_done move_tab
  toggle_pin drop_tab(id, target, DropSide) focus_pane cycle_pane cycle resize_split(path, gutter,
  delta, total) reveal_tab look set_panes`.
- Panel hotkeys (chord ids): `set_hotkey(id, Option<&str>)` steals from other panels with a banner,
  `release_hotkey(chord)` (an action took it), `retain_hotkeys(keep)` (load-time drop of action-owned keys),
  `panel_for(accepts) -> Option<id>`; free fn `hotkey_label(id)`.
- History: `standing_of` (on screen / background / live / gone / fresh) and `resume_chat`, which attaches a
  background session in a shell running `claude attach '<job id>'`.
- Readers/setup: `show_doc(Doc, DocPlace) show_plan show_message show_file follow_link open_markdown
  show_setup set_setup_selected`.
- Groups: `save_group group_panes ungroup remove_group clear_groups rename_group open_group`.
- Launchers: `add_launcher edit_launcher remove_launcher`. Hibernation: `hibernate wake hibernate_idle`.
- Git: `refresh_git new_task(NewTask) remove_worktree prune_worktrees open_in_editor`.
- History: `all_chats() chats_in(root)` (`'static` futures) `folder_at standing_of resume_chat`.
- Search: `set_query toggle_filter toggle_filter_folder toggle_filter_feature clear_search`.
- Bell: `show_banner show_banner_sticky clear_banner dismiss_toast hold_toast read_notices_of toggle_notices
  close_notices toggle_sessions close_sessions open_session open_notice dismiss_notice read_all_notices
  clear_notices set_window_active see_focused`.
- Ingest: `apply(StoreMsg)`, `apply_hook apply_status apply_agents`.
- Flows: `queue start_flow hold_for pump_flows advance descends_from`; free `shell_label handoff_text`.
- Suggestions: `sink() -> Arc<dyn SuggestionSink>` (hand to `HookServer::set_sink`), `suggest withdraw
  pending_suggestion settled_as suggestion_of dismiss_suggestion run_suggestion_here start_suggestion_here
  track_suggestion untrack_suggestion start_suggestion_in_plugin suggestion_taker`.

## Plugins (spec 04)
- `attach_plugins(Plugins)` installs `HostDispatcher(PluginHost)` + a listener; `plugin_host()` implements
  `PluginHostApi` by running each call on the store via `StoreMsg::Run`.
- Views: `plugin_view(tab)`, `modal()`, `close_modal() dock_modal()`, `sidebar_view_of(plugin)`,
  `plugin_ui()` (views, modal, sidebars, badges, floats), `move_float(key, FloatSpot)`, `view_action(tab, action, values)`.
- Dialogs: `plugin_dialog() -> Option<&PluginDialog{plugin, ask: DialogAsk::{Pick, Input, Form}}>`,
  `answer_dialog(DialogAnswer::{Text, Form})`.
- Rail: `rail_plugins() move_rail_plugin footer_commands rail_badge open_windows show_sidebar_view`.
- `run_plugin_command(full_id, on) set_plugin_enabled uninstall_plugin set_plugin_setting`.
- Sessions API (spec 06 F): `portrait_of(tab)`; `session.status` on hibernate/wake/settle, `session.renamed`
  (plugin renames only — the sidebar's `rename_tab` must call `emit_renamed`), `session.context`; the launch
  settle waits out the folder-trust prompt (2 s × 300); alerts of a tab with a portrait carry it.
- Not yet: plugin-owned screens (`contributes.screen`) — every view lives in the sessions grid.

## Test fakes (`testing`)
`FakeClock`, `FakeTerms` (+`TermLog`, `give_pid`, `exit`), `FakeNotifier`, `FakeBus`, `FakeHooks`.
