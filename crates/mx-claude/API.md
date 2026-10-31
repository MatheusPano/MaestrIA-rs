# mx-claude — API

Spec: `docs/spec/03-claude-integration.md`. State dir is always a parameter (never `~/.maestria`).

## Loopback server (`server`)
- `HookServer::start(state_dir, UnboundedSender<ServerEvent>) -> io::Result<HookServer>` (async):
  binds `127.0.0.1`, reuses `<state_dir>/hook-port` when free, rewrites it otherwise, and
  (re)writes `<state_dir>/statusline.sh`. Routes: `/mcp/<tab>` (POST only), `/status/<tab>`,
  anything else = hook. Hooks/status always answer `200 {}`. Any request with `Origin`, or a
  `Host` other than `127.0.0.1:<port>`/`localhost:<port>`, gets `403`; bodies capped at `MAX_BODY_BYTES`.
- `.port()`, `.running()`, `.stop().await` (abort; also on drop), `.status_line_file()`,
  `.settings_for(tab, cwd, home)`, `.mcp_config_for(tab)`, `.set_sink(Arc<dyn SuggestionSink>)`
  (before it is set, `NoSink` answers "no panel"), `.set_plugin_tools(Arc<dyn PluginTools>)` (default
  `NoTools`; its names also go into `.settings_for`'s allow-list).
- `ServerEvent::Hook { tab_id, name, payload }` (raw payload for mx-core's reducer) and
  `ServerEvent::Status { tab_id, payload }` (feed to `ContextUsage::from_status_line`).
- `STATUS_LINE_SCRIPT` (byte-identical), `port_file(dir)`, `status_line_file(dir)`.

## Launch (`launch`, `sh`)
- `settings_for(port, status_line_file, tab, cwd, home) -> String` — exact `--settings` JSON.
- `settings_with_tools(.., plugin_tools: &[String])` — same, plus `mcp__maestria__<tool>` per plugin tool.
- `mcp_config_for(port, tab) -> String` — exact `--mcp-config` JSON.
- `user_status_line(cwd, home) -> Option<Map>` — local > project > home precedence.
- `ClaudeLaunch { port, mcp_config, settings, name, brief, project, resume_id, prompt }`
  `.command_line()` (pty command, `--mcp-config` first) / `.display_line()` (shown text).
- `sh_quote(s)` — POSIX single-quote.

## MCP (`mcp`)
- `trait SuggestionSink: Send + Sync` — implemented by the store:
  `suggest(NewSuggestion{tab_id,title,tldr,prompt}) -> BoxFuture<Option<SuggestionQueued>>`
  (None = tab gone / not Claude), `withdraw(tab_id, task_id) -> BoxFuture<Withdrawal>`.
- `SuggestionQueued { task: TaskRef, pending: Vec<TaskRef>, evicted: Vec<TaskRef> }`,
  `TaskRef { id, title }`, `Withdrawal { withdrawn, already: Option<SuggestionEnd> }`,
  `SuggestionEnd::{Started, Dismissed}`, `NoSink`.
- `trait PluginTools: Send + Sync` — `list() -> Vec<Value>` (MCP descriptors), `call(tab, name, args)
  -> BoxFuture<Option<Value>>` (None ⇒ `-32602 unknown tool`); `NoTools`.
- `new_task_id()` (`task_` + 8 hex, CSPRNG), `is_task_id`, `MAX_PENDING = 20`,
  `MAX_PROMPT = 32000` (UTF-16), `PROTOCOL_VERSION`, `SERVER_NAME`, `tool_names()`,
  `tool_name(tool)` (`mcp__maestria__<tool>`).
- `handle(&Value, tab, sink) -> Option<Value>` (None ⇒ HTTP 202), `handle_with(&Value, tab, sink, tools)`
  (built-ins then plugin tools), `handle_body(&[u8], tab, sink, tools)`,
  `parse_error()`, `tools_json()`, `tool_error(text)` (`isError` result), `INSTRUCTIONS`.

## Context (`context`)
- `ContextUsage { percent, tokens, window }::from_status_line(&Value) -> Option<_>`;
  `.chip_filled()` (0..=5), `.chip_level() -> ChipLevel::{Calm, Warn, Danger}`
  (fgDim / yellow / red), `.chip_text()`, `.chip_tooltip()`.
- `format_tokens(i64) -> String` (`116 mil`, `1,2 mi`).

## Plan usage (`usage`)
- `read_usage(claude_dir, claude_json).await -> UsageReading` (real endpoint; token from
  `<claude_dir>/.credentials.json`, macOS keychain fallback; account from `claude_json`).
- `fetch_usage(endpoint, Option<token>, account_fn).await` — same with injectable endpoint.
- `parse(body, UsageAccount) -> UsageReading { windows, account, credits, problem }`,
  `.ok()`; `UsageWindow { label, percent, resets_at: Option<DateTime<Local>> }`;
  `UsageAccount { email, organization, plan }` `.is_empty()`, `.headline()` (`org · plan`).
- `access_token(dir)`, `token_in(raw)`, `account(path)`, `plan_name`, `money(&Value)`,
  `short_until(&at, &now)`; error texts `NO_TOKEN`, `EXPIRED`, `OFFLINE`, `TOO_SLOW`,
  `NOT_JSON`, `NO_LIMITS` (others are formatted: `A api respondeu N`, `Não deu pra ler: e`).

## Setup pane model (`setup`)
- `SetupSection` (`ALL` in display order) `.label() .about() .help() .open() .noun()`,
  `SetupSection::of_rel(rel)`.
- `SetupSlot { section, rel, label, about, exists, fixed }` `.is_json()`.
- `scan(root) -> Vec<SetupSlot>`, `phantom_slot(root, rel)`, `read(root, rel)` (lossy UTF-8,
  missing ⇒ ""), `write(root, rel, text)`, `remove(root, rel)` (skill ⇒ whole dir), `path_of`.
- `slug(name)`, `next_adr(root)`, `rel_for(section, name, root)`, `template(rel, folder, title)`.
- `MxSetup { root, selected }` `.name() .to_json() ::from_json`.

## Reader model (`docs`, `markdown`)
- `DocSource::{Plan, File, Message, Report, Plugin}` `.name() .label() .on_disk() ::from_name`.
- `MxDoc { source, title, text, path, origin, at: NaiveDateTime }` `::new ::file`,
  `.missing() .reload() .identity() .to_json() ::from_json` (text clipped at 128 Ki UTF-16).
- `is_markdown_path`, `expand_home(path, home)`, `resolve_link_path(path, base, home)`,
  `decode_href(href)` (reader's percent-decode), `follow_link(href, base, home) -> LinkAction
  ::{Nothing, OpenExternal, Missing, Read, QuickLook}` (`.problem()` = banner text).
- `parse_markdown(text) -> MdNode { kind: MdKind, children }` — comrak GFM (tables, tasklist,
  strike, autolink, footnotes, alerts, shortcodes, heading ids); `.plain_text()`;
  `markdown_options()`.
