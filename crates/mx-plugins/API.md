# mx-plugins — API pública

Host de plugins sem UI (spec 04 §0–§7). Tokio. RFW não é interpretado aqui: `rfw_library`/`rfw_data`
ficam opacos em `PluginView` e a UI os entrega ao `mx-rfw`.

## Constantes (raiz)
`PLUGIN_API=1`, `BLOCKS_VERSION=2`, `MANIFEST_NAME`, `SIDEBAR_VIEW_ID="sidebar"`, `LOG_CAP=400`,
`MAX_SIDEBAR_COMMANDS=3`. `MAESTRIA_COMPAT="2.7.0"` (versão da MaestrIA que este host diz ser nas comparações
de compatibilidade, ex. o mínimo do catálogo). `CAPABILITIES: [(chave, valor, landed)]` = as flags do
`initialize` na ordem do 2.7; só as `landed` vão no fio — quem entrega um widget liga a dele aqui.

## Manifesto (`manifest`)
- `PluginManifest::parse(&Obj)` / `parse_for(&Obj, linux)` / `read(dir)` → `Result<_, ManifestError(String)>`
  com as mensagens verbatim e a ordem de erro do Dart. `has_process()`, `activates_on(ev)`, `allows(perm)`.
- `PluginCommand { plugin_id, id, title, target, run, send, key: Option<Chord>, icon, sidebar }`,
  `full_id()`, `short_title(plugin_name)`. `CommandTarget { Plugin, Terminal, Background, Session }`.
- `PluginSetting { id, title, kind: SettingType, description, default, options, group }`, `resolve(saved)`;
  `SettingType::Folders` = lista de `root`s (`[]` quando o salvo não é lista de textos).
- `Permission { Hooks, TerminalWrite, SessionsCreate, SessionsClose, Notifications }`: `id()`, `label()`, `by_id()`.

## Teclas (`chord`)
`Key`, `Chord { key, meta, control, alt, shift }`: `parse("meta+alt+g")`, `for_host(linux)` (meta→Ctrl+Shift),
`label(linux)`, `rejection(linux)`, `accepts(&KeyPress)`, `hits(&KeyPress)` (fallback físico a–z/0–9).
`KeyPress { key, physical: Option<u32> /* USB HID usage, página 7: a=0x04 */, meta, control, alt, shift }`.

## Registro (`Plugins`, handle `Clone`)
- `Plugins::new(state_dir)` → `<state_dir>/plugins` e `<state_dir>/plugin-data/<id>`; `with_dirs(root, data)`.
  `set_calls(Arc<dyn PluginCalls>)` (normalmente `HostDispatcher`), `set_listener(Arc<dyn PluginListener>)`
  (`changed`, `ready(id)` — mande `sidebar.shown` daí —, `logged(id)`), `set_env(vars)` (PATH aumentado),
  `set_runtime(Handle)` quando criado fora do tokio.
- Leitura: `scan()`, `all()/get(id) -> PluginInfo`, `log_of(id)`, `note(id, line)`, `themes() -> mx_theme::Themes`,
  `commands()`, `sidebar_commands()`, `command_by_id(full)`, `command_for(&KeyPress, app_keymap_hit)`,
  `busy()/plugin_busy(id)/set_busy`, `suggestion_takers()`, `is_up(id)`, `disabled()`.
- Persistência (chave `plugins` do config): `load(&Value)`, `to_json()`, `settings_of(id)`,
  `set_setting(id, key, Option<Value>)` (manda `settings.changed`).
- Processo: `startup()`, `start(id).await`, `stop(id)`, `stop_all()`, `kill_all()`, `restart(id)`,
  `set_enabled(id, on)`, `wake(id)`, `tell(id, type, data)`, `emit(type, data, activations, needs)`
  (também `mx_core::ports::PluginBus`), `view_action(plugin, view, action, values)`,
  `invoke(&cmd, context) -> Result<Value, RpcError>`, `offer_suggestion(id, params)`.
  Regras: só `onStartup`/enable/install/link sobem sozinhos; eventos nunca revivem um crash; comandos,
  `suggestion.start`, `wake` e `restart` revivem. Timeouts 10 s/30 s; shutdown 0,8 s + SIGTERM 0,8 s + SIGKILL.
  `view_closed(plugin, view)` = evento `view.closed` só pro dono, só se ele está rodando (também em
  `PluginBus::view_closed`, que é o que a store chama ao fechar o modal).
- Instalação: `stage(src) -> StagedPlugin`, `commit(&staged)`, `discard(&staged)`, `link(path)`, `uninstall(id)`;
  `InstallError(String)`; textos `registry::{trust_headline, trust_facts, installed_banner}`.
- `PluginInfo { id, dir, data_dir, manifest, problem, linked, enabled, crash, state, palettes }`,
  `PluginState` (+`label()`).

## RPC (`rpc`)
- `RpcError { code, message }` com construtores das mensagens verbatim (`no_tab`, `no_focused_tab`,
  `no_session_on_screen`, `no_process(title)`, `no_window`, …). `rpc::wire` = enquadramento compacto.
- `PluginHostApi` (a store implementa; 37 métodos, todos `HostResult<'_, T>` = BoxFuture): banner, notify,
  open_url, pick, input, form, clipboard_write, sessions, focused_session, focus_session, folders,
  features_or_hotfixes, workspaces, chats, open_in_editor, send_text, send_keys, paste_clipboard,
  open_claude, open_shell, rename_session, wake_session, hibernate_session, set_portrait, tab_owner(id),
  close_tab, suggestion_created, open_markdown, view_open/update/append_lines/clear_lines/close,
  sidebar_update, float_show/update/hide. Params tipados: `SendText`, `SendKeys { keys: None = lista
  inválida }`, `OpenClaude { background }`, `OpenShell`, `ViewOpen { from, width, height }`, `ViewUpdate`,
  `SidebarUpdate { badge: BadgeUpdate }`, `FloatContent`, `SuggestionCreated`, `RfwUpdate`,
  `TabOwner { tab_id, kind, owner }`.
- `rpc::keys::{parse_keys, bad_keys, MAX_KEYS}` (nomes ↔ `mx_core::ports::TermKey`);
  `rpc::portrait::{resolve_portrait, not_claude}` (realpath dentro de pluginDir/dataDir, senão -32001).
- `HostDispatcher::new(api)` (+`with_window_probe`) valida params/permissões e monta os resultados
  (`{tabId}`, `{open}`, `{shown}`…). `log`, `settings.get` e `command.busy` são atendidos pelo registro.
- `host_json::{SessionJson, FolderJson, FeatureJson, WorkspaceJson, ChatJson}` → `to_json` no formato §2.5
  (+ `context` antes de `owner`); `context_json(Option<&ContextUsage>)` é o mesmo formato do evento.

## Modelos de UI (`ui`, `floats`, `rail`, `commands`, `suggestions`)
- `ui::view::PluginView` (blocos + RFW opaco, `set_rfw` merge raso, `apply_open` não limpa a lib,
  `apply_update` com blocos limpa, consoles adotados, `terminal_tabs()`); `ui::console::PluginConsole`.
- `ui::blocks::Block::parse` (todos os tipos §3.1, `Unknown`/`Broken`), `ui::menu`, `ui::fields::FieldState`
  (§3.2 `sync`/`values`), `ui::icons` (`material_icon`, `glyph`, `svg_of`, `Tone`, `plugin_color`),
  `ui::calendar` (`Calendar`, textos pt-BR), `ui::form::PluginForm` (`parse`, `rows`, `press`, `enter_button`),
  `ui::dialogs::{PickRequest, InputRequest, FormAnswer, cycle_highlight}`.
- `floats::PluginFloats` (show/update/hide, clamps, `FloatSpot::dropped/origin`, `to_json/read_json`).
- `rail::{rail_plugins, move_rail_plugin, footer_commands, rail_badge, rail_tooltip, sidebar_placeholder}`.
- `commands::{CommandValues, expand, sh_quote, plan -> CommandRun, background_banner, no_session_banner}`.
- `suggestions::{start_params, suggestion_prompt, worktree_branch, worktree_dir, gone_banner, SuggestionTracker}`.
