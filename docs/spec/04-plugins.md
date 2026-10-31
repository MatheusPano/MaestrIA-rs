# 04 — Plugin system

Source of truth (Flutter app, read-only): `repo/lib/services/plugins.dart`,
`plugin_api.dart`, `plugin_floats.dart`, `repo/lib/ui/plugin_{rfw,pane,dialogs,modal,float}.dart`,
plugin bits of `repo/lib/services/store.dart`, `docs/plugins.md`, `docs/criando-um-plugin.md`.
Real plugins surveyed: `maestria-plugins/*` and `argocd-plugin`.

Compatibility goal: **every existing plugin must run unchanged**. Where docs and code
disagree, the code wins and the discrepancy is called out with **NOTE**.

User-visible strings are Portuguese and are quoted verbatim; plugins and tests depend on
several of them (error messages travel back to plugins inside JSON-RPC errors).

---

## 0. Constants

| name | value | meaning |
|---|---|---|
| `mxPluginApi` | `1` | contract version (`maestria` field, `MAESTRIA_API` env, `initialize.apiVersion`) |
| `mxPluginBlocks` | `2` | blocks generation (`initialize.blocks`) |
| `mxManifestName` | `"maestria-plugin.json"` | manifest file at plugin root |
| plugins root | `~/.maestria/plugins/` (`$mxStateDir/plugins`) | one dir (or symlink) per plugin, named by id |
| data dir | `~/.maestria/plugin-data/<id>/` | survives updates; deleted on uninstall |
| staging | `~/.maestria/plugins/.staging/p<random>/src` | temp install area |
| log cap | 400 lines per plugin | oldest dropped |
| sidebar footer command cap | 3 | `Plugins.maxSidebarCommands` |
| reserved viewId | `"sidebar"` | `AppStore.sidebarViewId` |

---

## 1. Manifest, install, lifecycle, permissions, activation

### 1.1 Manifest schema (`maestria-plugin.json`)

Must be a JSON object. Read errors → plugin state `invalid`, `problem` = message:
- file missing: `não tem maestria-plugin.json`
- bad JSON: `maestria-plugin.json não é um json válido: <parser message>`
- not an object: `maestria-plugin.json precisa ser um objeto`

Top-level fields:

| field | type | required | rules / errors (FormatException text verbatim) |
|---|---|---|---|
| `id` | string | yes | trimmed; non-empty else `"id" é obrigatório`; must match `^[a-z0-9][a-z0-9._-]*$` else `"id" só pode ter minúsculas, dígitos, ponto, hífen e sublinhado: "<id>"` |
| `version` | string | yes | trimmed non-empty else `"version" é obrigatório` |
| `name` | string | no | trimmed; empty/missing → `id` |
| `description` | string | no | default `""` |
| `author` | string \| `{ "name": string }` | no | anything else → `""` |
| `maestria` | number | no | `toInt()`; default 1; `> 1` → `o plugin pede a versão <n> da API e esta MaestrIA só conhece até a 1` |
| `main` | string \| string[] | no | string (non-blank, trimmed) → `[s]`; non-empty list of strings → as is; anything else (incl. `[]`, `""`, list with non-strings) → `"main" é uma lista: ["node", "main.js"]` |
| `icon` | string | no | icon name (§3.6) or `.svg` path relative to plugin dir; non-string ignored |
| `activationEvents` | string[] | no | non-strings silently dropped; stored as a set |
| `permissions` | string[] | no | each must be a known id else `permissão desconhecida: "<p>" — as que existem são hooks, terminal.write, sessions.create, notifications` (a non-string prints via Dart toString) |
| `contributes` | object | no | see below; non-map → treated as `{}` |

`contributes` keys: `commands`, `themes`, `settings`, `sidebar`, `screen`, `suggestions`, `tools`.
Any other key → warning `"contributes.<k>" não existe nesta versão — ignorado` (not fatal).

#### `contributes.commands[]`

| field | rule |
|---|---|
| (item) | must be an object, else `cada comando é um objeto` |
| `id` | string matching `^[A-Za-z0-9._-]+$` else `comando com "id" inválido: <jsonEncode(id)>` (e.g. `null`, `"a b"`) |
| `title` | non-blank string else `o comando "<id>" precisa de "title"`; stored trimmed |
| duplicate id | `comando repetido: "<id>"` |
| `run` | string → shell command |
| `send` | string → text sent to focused claude session |
| `run`+`send` | `o comando "<id>" tem "run" e "send" — escolha um` |
| `in` | only with `run`: `null`/`"terminal"` → terminal, `"background"` → background, else `o comando "<id>" tem "in": "<v>" — vale "terminal" ou "background"` |
| neither run/send | target = plugin process; requires `main` else `o comando "<id>" não tem "run" nem "send", e o plugin não tem "main" pra atendê-lo` |
| `key` | string chord (§4.6). Unparseable → warning `o comando "<id>" pede a tecla "<k>", que não dá pra ler — ficou sem`; rejected → warning `o comando "<id>" pede <label>: <why> — ficou sem`. Command kept without key. |
| `icon` | string, optional |
| `sidebar` | `true` → footer button (only literal `true`) |

Command targets: `plugin`, `terminal`, `background`, `session`. Full id = `"<pluginId>/<commandId>"`.

#### `contributes.themes[]`
Each item is a string path or `{ "path": string }`, else `cada tema é {"path": "themes/x.json"}`.
Themes are loaded at scan time (§4.7); a bad theme file is only logged.

#### `contributes.settings[]`
Item must be an object (`cada configuração é um objeto`); duplicate id → `configuração repetida: "<id>"`.

| field | rule |
|---|---|
| `id` | non-empty string else `cada configuração precisa de "id"` |
| `type` | `string` (default), `number`, `boolean`, `color`, `select`; else `a configuração "<id>" tem "type": "<t>" — vale string, number, boolean, color, select` |
| `title` | default = id |
| `description` | default `""` |
| `default` | any JSON |
| `group` | string, header shown above consecutive settings with same group |
| `options` | list of string or `{value: string, label?: string}`; other → `a opção de "<id>" é um texto ou {"value", "label"}`; `select` with no options → `a configuração "<id>" é select e não tem "options"` |

Value resolution (`resolve(saved)`): if `saved` fits the type use it; else if `default` fits use it;
else type-empty: boolean `false`, number `0`, select first option value, string/color `""`.
"Fits": string/color → is string; number → is number; boolean → is bool; select → string equal to
an option value. Only user-changed values are persisted (§1.6).

#### `contributes.sidebar`
`true`/`false`; non-bool → `"contributes.sidebar" é true ou false`; `true` without `main` →
`"contributes.sidebar" precisa de "main" pra desenhar a aba`.

#### `contributes.screen`
`null`/`false` → none; `true` → screen without home; `{ "home": "<commandId>" }` → home must be a
declared command else `"contributes.screen.home" aponta pro comando "<home>", que o manifesto não declara`;
other → `"contributes.screen" é true ou um objeto com "home": {"home": "quadro"}`. Screen without
`main` → `"contributes.screen" precisa de "main" pra abrir as janelas`.
**NOTE**: an object without `home` (e.g. `{}`) hits the error branch.

#### `contributes.suggestions`
`{ "label": non-blank string }` (trimmed) else `"contributes.suggestions" é um objeto com "label": {"label": "criar a task no …"}`;
without `main` → `"contributes.suggestions" precisa de "main": quem recebe a sugestão é o processo`.

#### `contributes.tools[]` (Rust port only)
MCP tools the plugin offers to the Claude sessions through the per-tab `maestria` MCP server
(03-claude-integration §2.7). `null`/absent → none.

| field | rule |
|---|---|
| (list) | must be a list of objects, else `"contributes.tools" é uma lista: [{"name": "x_abrir", "description": "…", "inputSchema": {"type": "object"}}]` |
| `main` | a non-empty list needs `main`, else `"contributes.tools" precisa de "main": quem responde a ferramenta é o processo` |
| `name` | matches `^[a-z][a-z0-9_]{0,47}$` else `nome de ferramenta inválido: "<name>" — minúsculas, dígitos e sublinhado, começando por letra, até 48 caracteres`; `suggest_task`/`dismiss_task` → `"<name>" é uma ferramenta da MaestrIA — escolha outro nome`; must start with `<last "."-segment of the plugin id>_` (`maestria.design` → `design_`) else `a ferramenta "<name>" tem que começar com "design_"`; duplicate → `ferramenta repetida: "<name>"` |
| `description` | string, trimmed, default `""` |
| `inputSchema` | object with `"type": "object"` (kept verbatim), else `a ferramenta "<name>" precisa de "inputSchema" com "type": "object"` |

Checks run per item in that order (name, reserved, prefix, duplicate, schema). A plugin id with a `-`
in its last segment cannot declare tools (the prefix can never match the name pattern).

Parse order matters only for which error wins: id → maestria → main → permissions →
activation → commands → themes → settings → sidebar → screen → suggestions → tools → unknown keys → version.
**NOTE**: `version` is checked last (inside the constructor call), so a manifest missing both
`version` and having a bad command reports the command error.

### 1.2 Plugin states

`invalid` ("Com problema"), `disabled` ("Desligado"), `idle` ("Pronto"), `starting` ("Subindo"),
`running` ("Rodando"), `crashed` ("Parou"). Derivation (in order):
manifest null → invalid; !enabled → disabled; connection ready → running; spawn in flight →
starting; `crash != null` → crashed; else idle.

`active` = manifest != null && enabled.

Settings card color: running green; crashed/invalid red; starting yellow; disabled faint; idle dim.

### 1.3 Directory scan (`scan()`), synchronous

1. List `root` (no follow); keep entries not starting with `.` that are directories or symlinks
   to directories; sort by full path (→ alphabetical by dir name).
2. For each: read manifest; FormatException/FileSystemException → `problem`.
3. Duplicate id (another earlier entry has the same manifest id) → manifest null, problem
   `outro plugin já usa o id "<id>"`.
4. Reuse the previous `MxPlugin` object for the same path (keeps the running process). If the
   id or the `main` list changed, stop the old process.
5. `enabled = !disabled.contains(id)`; each manifest warning logged as `aviso: <w>`.
6. Plugins whose dir disappeared are stopped.
7. Rebuild plugin themes (§4.7), notify.

`id` of an invalid plugin = last path segment of its dir.

### 1.4 Install / update / dev-link / uninstall

**Stage** (`stage(source)`):
- `src = expandHome(source.trim())` with trailing `/` removed; empty → `Diga de onde instalar`.
- create `root/.staging`, temp dir `p*`, target `<tmp>/src`.
- git if source starts with `https://`, `http://`, `git@`, `ssh://` or ends with `.git`:
  `git clone --depth 1 '<src>' '<into>'`.
- `.zip` (case-insensitive) that exists: macOS `ditto -x -k`, else `unzip -q <src> -d <into>`.
- existing directory: `cp -R <src> <into>`.
- else `"<source>" não é uma URL de git, um .zip nem uma pasta que exista`.
- command failure → last non-blank stderr line, or `Falhou (código <n>)`.
- manifest dir: `<into>` if it has the manifest; else if exactly one non-hidden, non-`__MACOSX`
  subdirectory has it → that subdir; else `Não achei um maestria-plugin.json ali`.
- manifest parse error → its message. Any error deletes the staging dir.
- Result carries `replacing` = version of the installed plugin with the same id (if any).

**Trust screen** (before commit) shows: `"<id> <version> · de <author> · no lugar da <old>"`,
description, then facts:
`Roda um programa seu, com as suas permissões: <main joined by space>` or
`Só declarações — nenhum programa roda`; per permission `Pode <label>`;
`Comandos: <title> (<key label>), …`; `Um tema` / `<n> temas`; each warning.
Dialog title `Instalar plugin` / `Instalar <name>?` / `Atualizar <name>?`; buttons `Cancelar`,
`Continuar`, `Voltar`, `Confio, instalar` / `Confio, atualizar`. Banners after install:
`<name> <version> instalado` / `<name> atualizado de <old> pra <new>`.

Permission labels:
- hooks: `ver os eventos das sessões do Claude — o que você pede, que ferramentas rodam e com quais argumentos`
- terminal.write: `digitar e enviar texto nos seus painéis`
- sessions.create: `abrir sessões do Claude e terminais`
- notifications: `mandar notificações do sistema`

**Commit**: remove old plugin with same id (stop process, delete dir or unlink), `mkdir -p root`,
rename staged dir to `root/<id>`, delete staging, `scan()`, log `instalado de <source>`, start if
`activatesOn('onStartup')`. Store then closes all views/modal/floats of that plugin.

**Discard**: delete staging dir (also when the dialog is dismissed).

**Dev link** (`link(path)`): path must be a dir (`"<path>" não é uma pasta`); manifest must parse;
id must not be installed (`Já existe um plugin "<id>" — remova ele antes`); create symlink
`root/<id>` → absolute src; scan; log `carregado de <src> (desenvolvimento)`; start if onStartup.
Banners: `<name> carregado de <basename> — reinicie pra pegar mudanças` /
`Não deu pra carregar: <msg>` (sticky). `linked` = entry is a symlink.

**Uninstall**: close views; stop; delete dir (or only the symlink); delete data dir; remove from
`disabled`; scan. Confirm text for linked: `Sai da MaestrIA o link pra <basename>. A sua pasta de desenvolvimento fica onde está, intocada.`
otherwise `A pasta do plugin e o que ele guardou vão embora. Pra ter de volta, instale de novo.`

**Enable/disable** (`setEnabled`): updates `disabled` set (persisted), disabling stops the process
and the store closes views/modal/floats; clears `crash`; reloads themes; enabling starts if onStartup.
If the active theme belonged to the plugin, fall back to the default theme.

**Restart**: stop, clear crash, full `scan()`, start if active and has a process.

### 1.5 Activation events

`activatesOn(event)`: false without process; with empty `activationEvents` → only `onStartup`;
`*` → always; else set membership.

| event string | emitted when |
|---|---|
| `onStartup` | app start (`startup()`), enable, install, link |
| `onCommand:<id>` | **NOTE**: never consulted by code. Commands always start the process via `invoke` regardless. It is only declarative. |
| `onHook:<Name>`, `onHook:*` | hook event with that name (requires `hooks` permission to be delivered at all) |
| `onSession` | `session.opened/closed/exited/status` |
| `onSuggestion` | **NOTE**: not consulted; `suggestion.start` is a request that always starts the process |
| `onTool:<name>` | declarative, like `onCommand:`: a `tool.call` for one of the plugin's tools always starts the process |
| `sidebar.shown` | not an activation event; the sidebar tab calls `wake()` directly |

Event delivery (`emit(type, data, activations, needs)`): for each active plugin with a process,
skipped if it lacks `needs`; if not up (no live conn and not starting) it is skipped when it has
`crash != null` **or** none of `activations` activates it; otherwise `_start` then send
notification `event` with `{type, ...data}` once ready. **A crashed plugin is never revived by events.**

### 1.6 Persistence (window config)

Under key `plugins` in the app config:
```json
{ "disabled": ["id1","id2"], "settings": { "<pluginId>": { "<settingId>": <value> } } }
```
`disabled` sorted, omitted when empty; plugins with empty settings maps omitted; whole key omitted
when empty. `setSetting(id, null)` removes the override (back to default). Also persisted
elsewhere: `suggestionsToPlugin: true`, rail order, sidebar view, float spots (§4.1).

### 1.7 Process spawn

- Executable resolution (`resolveExecutable(exe, dir, PATH)`): `expandHome(exe)`; contains `/` →
  absolute or `dir/exe`, must exist; bare name → first `PATH` entry containing it (empty entries
  skipped). Not found → crash `não achei "<exe>" — nem na pasta do plugin, nem no PATH`.
  PATH is the app's augmented PATH (includes `/opt/homebrew/bin` etc., same as for `claude`).
- cwd = plugin dir; args = `main[1..]`.
- env = app env + `MAESTRIA_API=1`, `MAESTRIA_PLUGIN_ID`, `MAESTRIA_PLUGIN_DIR`, `MAESTRIA_PLUGIN_DATA`.
- data dir created (`mkdir -p`) before spawn.
- log `processo <pid> subiu: <main joined>`.
- send `initialize` (10 s timeout). On success: `ready = true`, crash cleared, `onReady` fired in a
  microtask (store uses it to send `sidebar.shown` if the plugin's tab is live).
- Failures → `_fail`: crash = reason, kill, log. Reasons: `o initialize falhou: <msg>`,
  `não deu pra rodar: <msg>` (spawn error), or `<error>`.
- Concurrent starts share one in-flight future (`_starting`), so two requests never spawn twice.
- Unexpected exit: crash = `o processo saiu (código <n>)`, busy flags of the plugin cleared, logged;
  pending requests fail with `-32002 "o plugin saiu"`.

**Stop** (`close`, grace 800 ms): send notification `shutdown`, close stdin, wait 0.8 s for exit,
then SIGTERM, wait 0.8 s, then SIGKILL. Log `processo parado`. Stop clears busy flags.
App quit: `stopAll()` in parallel; `killAll()` (SIGKILL) on dispose.

Log line format: `HH:MM:SS  <text>` (two spaces). Lines from stderr: `stderr: <line>`; unparseable
stdout: `stdout: <line>`; `log` method: raw message; command run: `comando <id>`; failure:
`comando <id> falhou: <msg>`; key hit: `tecla <label> → <id>`; suggestion: `sugestão <id>`.

### 1.8 Permissions model

Not a sandbox; gates what the host does on request.

| permission | gates |
|---|---|
| `hooks` | receiving `event{type:"hook"}`; `chats.list`; `activity` field in session JSON |
| `terminal.write` | `terminal.sendText` |
| `sessions.create` | `session.openClaude`, `session.openShell` |
| `notifications` | `window.notify` |

Missing → error `-32001` with message `o manifesto não declara a permissão "<id>"`.

---

## 2. JSON-RPC protocol

### 2.1 Framing

- JSON-RPC 2.0, UTF-8, **one JSON object per line** (`\n`), both directions. Host writes
  `jsonEncode(msg)` + `\n` to the plugin's stdin.
- Host message shapes: request `{"jsonrpc":"2.0","id":<int>,"method":..,"params":{..}}` (params key
  omitted when null); notification without `id`; response `{"jsonrpc":"2.0","id":<echoed>,"result":<v>}`
  (result may be `null`) or `{"jsonrpc":"2.0","id":..,"error":{"code":int,"message":string}}`.
- Host request ids are integers starting at 1 per connection.
- Reading plugin stdout: blank lines ignored; non-JSON or non-object lines → plugin log `stdout: <line>`.
  `params` that is not an object → `{}`.
  - `method` string + `id` absent/null → notification.
  - `method` string + `id` (any JSON type, echoed back as-is) → request.
  - otherwise a response: only integer ids matched; unknown ids ignored; `error` map → failure with
    `code` (default -32603) and `message` (default `erro sem mensagem`).
- Plugin notifications are handled like requests but the result/error is discarded. Any method
  (e.g. `window.showBanner`) may be sent as a notification.
- Host exceptions while handling a plugin request: `PluginRpcError` → its code/message; any other
  exception → `-32603` with `"<exception toString>"`.
- Writes to a dead process are swallowed.

### 2.2 Error codes

| code | name | used for |
|---|---|---|
| -32601 | methodNotFound | `método desconhecido: <method>` |
| -32602 | invalidParams | validation |
| -32603 | internal | uncaught |
| -32001 | forbidden | permission / ownership |
| -32002 | unavailable | not running, timeout, no UI available |

Common messages: required string param missing/not string → `"<key>" é obrigatório` (-32602).
Before `onCall` is wired: `a janela ainda não está pronta` (-32002).

### 2.3 Host → plugin

| message | kind | timeout | params |
|---|---|---|---|
| `initialize` | request | 10 s | see below |
| `command.invoke` | request | 30 s | `{ command: "<commandId>", context: { cwd, folder, sessionId, title, pluginDir, tabId? } }` |
| `suggestion.start` | request | 30 s | `{ suggestion: { id, title, tldr, prompt }, choice: "worktree"\|"local"\|"here", origin: { tabId, cwd, root, folder, isRepo, branch } }` (`origin.folder` = folder *name*, `root` = folder root path) |
| `tool.call` | request | 120 s | `{ tool: "<name>", args: { ... }, tabId: "<tab of the calling session>", cwd: "<tab cwd>"\|null }` (Rust port only) |
| `event` | notification | — | `{ type, ... }` (§2.5) |
| `view.action` | notification | — | `{ viewId, action, values }` |
| `settings.changed` | notification | — | `{ settings: { <id>: <resolved value> } }` (all settings), only if ready |
| `shutdown` | notification | — | none (no `params` key) |

`initialize` params (exact keys):
```json
{
  "apiVersion": 1, "blocks": 2, "rfw": 2, "floats": 1, "input": 1, "form": 1, "modal": 1, "editor": 1,
  "sprites": 1, "zoom": 1, "textInput": 1, "openInBackground": 1, "markdown": 1, "pasteImage": 1, "keys": 1,
  "arrows": 1, "wake": 1, "hibernate": 1, "portrait": 1, "closeSessions": 1, "modalFrom": 1, "selectable": 1,
  "rename": 1, "picture": 1, "zoomFocus": 1,
  "pluginId": "<id>", "pluginDir": "<dir>", "dataDir": "~/.maestria/plugin-data/<id> (absolute)",
  "permissions": ["hooks", ...], "settings": { ... resolved ... }
}
```
Capability flags come from `CAPABILITIES` (`mx-plugins/src/registry/lifecycle.rs`), in that wire order.
`picture: 1` (Rust port only) means the `maestria.Picture` widget (§3.4 C.3) is available.
`zoomFocus: 1` (Rust port only) means `maestria.ZoomView` accepts `command: "focus"` and `onMove` (§3.4 C.3).
The result is ignored (any value). Errors → crash `o initialize falhou: <message>`.
Timeout message: `"initialize" ficou sem resposta por 10s` (generic: `"<method>" ficou sem resposta por <n>s`, code -32002).

`command.invoke` context values: `cwd` (tab cwd or folder root), `folder` (folder root),
`sessionId` (claude resume id or `""`), `title` (tab title or `""`), `pluginDir`, plus `tabId`
when a tab is in focus / the menu was opened on one. Result ignored. Errors become a sticky banner
`<command title>: <message>` and log `comando <id> falhou: <msg>`. If the plugin cannot start:
`-32002` with crash reason or `o plugin não subiu`; disabled: `o plugin não está ligado`.
`invoke` clears `crash` first (explicit user action revives crashed plugins).

`tool.call` (Rust port only, `Plugins::call_tool(tabId, cwd, name, args)`): sent to the first enabled
plugin whose `contributes.tools` declares `name`; none → `o plugin não está ligado`. Like `invoke` it
clears `crash`, logs `ferramenta <name>`, starts the process if needed and waits up to 120 s (cold-start
rendering). The result must be an MCP tool result (`{content: [...], isError?}`; `content` items may be
`{"type":"image","data":<base64>,"mimeType":"image/png"}`) and is handed to the session unchanged. An
error logs `ferramenta <name> falhou: <msg>` and reaches the session as `{"content":[{"type":"text","text":<msg>}],"isError":true}`.

`view.action` values: for block views, every field (`input` text, `checkbox` bool, `select`
value/null, `calendar` string or `{from,to}`) of the whole view (§3.5). For RFW views,
`values` = the event's argument map (converted to plain JSON). `viewId` = the view id, `"sidebar"`
for the sidebar tab, or the float id for floats. Only delivered if the plugin is active; it
starts the process if needed (and waits for ready).

### 2.4 Plugin → host methods (complete)

Params validated by `_text(key)` are required strings (`"<key>" é obrigatório`, -32602).

| method | params | result | notes / errors |
|---|---|---|---|
| `log` | `message` | `null` | handled before anything else; logged raw (`${message ?? ''}`) |
| `window.showBanner` | `text`*, `sticky?` | `null` | banner text `"<plugin name>: <text>"`; sticky only if `true` |
| `window.notify` | `title?`, `body`* | `null` | perm `notifications`; title default plugin name; OS notification |
| `window.openUrl` | `url`* | `null` | must start with `https://` or `http://` else -32602 `só dá pra abrir http(s)` |
| `window.pick` | `title?`, `placeholder?`, `items:[{value,label?,detail?}]` | chosen `value` or `null` | items without string `value` dropped; empty list → `null` immediately; no UI → -32002 `a janela não tem onde perguntar`; title default plugin name |
| `window.input` | `title?`, `placeholder?`, `value?`, `prompt?` | trimmed text or `null` (esc/outside/empty) | same -32002 |
| `window.form` | `title?`, `fields`, `buttons?`, `error?` | `{action, values}` or `null` | parse errors → -32602 with message (§4.4) |
| `settings.get` | — | resolved settings map | |
| `clipboard.write` | `text`* | `null` | |
| `sessions.list` | — | `[session]` | all tabs (incl. plugin views, readers, owned terminals) |
| `sessions.focused` | — | session or `null` | |
| `session.focus` | `tabId?` | `null` | no tabId → focused tab; unknown → -32602 `não há painel "<id>"` / `nenhum painel em foco`; switches screen to the tab's |
| `folders.list` | — | `[{name, root, isRepo, branch, worktrees:[{path,branch,isMain,label,prunable}], workspaces:[name]}]` | |
| `featuresOrHotfixes.list` | — | `[{id,name,kind:"feature"\|"hotfix",folder,brief}]` | |
| `projects.list` | — | same as above | legacy alias — keep |
| `workspaces.list` | — | `[{id,name,folders:[root],codeWorkspacePath?}]` | `codeWorkspacePath` omitted when null |
| `chats.list` | `day`* (`YYYY-MM-DD`) | `[{sessionId,title,cwd,folder,at(ISO),size}]` | perm `hooks`; unparseable → -32602 `"day" é AAAA-MM-DD` |
| `editor.open` | `path`*, `line?` | `null` | no line → app "open in editor"; with line → `code -g path:line`; failure → sticky banner `Não achei o vscode — nem o \`code\` no PATH, nem o app` |
| `command.busy` | `command`*, `busy` | `null` | command must be declared by the plugin else -32602 `o plugin não tem o comando "<id>"`; busy = (`busy == true`) |
| `terminal.sendText` | `text`*, `submit?`, `tabId?` | `null` | perm `terminal.write`. Target: tabId → that tab; else focused if not passive; else first on-screen non-passive claude tab; else first on-screen non-passive; else -32602 `nenhuma sessão na tela`. Passive/exited/hibernated target → -32602 `"<title>" não tem processo pra receber texto`. `submit:true` → type + Enter, else bracketed paste |
| `session.openClaude` | `cwd?`, `prompt?`, `label?` | `{tabId}` | perm `sessions.create`; place: cwd → folder containing it or loose; no cwd → focused tab's folder/cwd |
| `session.openShell` | `cwd?`, `command?`, `label?`, `owned?`, `tag?`, `embedded?` | `{tabId}` | perm `sessions.create`; `embedded` implies owned and the tab is not placed in the grid; owned → `owner = pluginId`, `ownerTag = tag` |
| `session.close` | `tabId?` | `null` | tab must be owned by caller else -32001 `só dá pra fechar um terminal que o plugin abriu com "owned"` |
| `suggestion.created` | `suggestionId`, `ref`, `intro?`, `branch?`, `dir?` | `{tabId, cwd}` or `null` | requires `contributes.suggestions` else -32001 `só quem declara "contributes.suggestions" recebe tarefas sugeridas`; missing id/blank ref → -32602 `faltam "suggestionId" e "ref"` |
| `pane.openMarkdown` | `markdown`*, `title?` | `{tabId}` | reader pane; title default plugin name; origin = plugin name |
| `view.open` | `viewId`*, `title?`, `blocks?`, `rfw?`, `data?`, `modal?` | `{tabId}` | `viewId == "sidebar"` → -32602 `"sidebar" é o id da aba da lateral — use sidebar.update` (§4.2) |
| `view.update` | `viewId`*, `title?`, `blocks?`, `rfw?`, `data?` | `{open: bool}` | `false` if no such view (closed by user) |
| `view.appendLines` | `viewId`*, `id`*, `lines` | `{open: bool}` | `viewId:"sidebar"` targets sidebar tab |
| `view.clearLines` | `viewId`*, `id`* | `null` | |
| `view.close` | `viewId`* | `null` | closes tab or modal |
| `sidebar.update` | `blocks?`, `rfw?`, `data?`, `badge?` | `{shown: bool}` | requires `contributes.sidebar` else -32602 `o manifesto não declara "contributes.sidebar"` |
| `float.show` | `id`*, `width?`, `height?`, `corner?`, `rfw?`, `data?`, `blocks?` | `{shown: true}` | bad corner → -32602 `"corner" é topLeft, topRight, bottomLeft ou bottomRight` |
| `float.update` | `id`*, `width?`, `height?`, `rfw?`, `data?`, `blocks?` | `{shown: bool}` | |
| `float.hide` | `id`* | `null` | |
| anything else | | | -32601 `método desconhecido: <method>` |

`rfw`/`data` parsing (shared): both absent → no rfw update. `rfw` present but not an object →
-32602 `"rfw" é { library, root? }`; `data` not an object → -32602 `"data" é um objeto`.
`rfw.library` and `rfw.root` optional strings (non-strings → **NOTE** Dart cast error → -32603).
`blocks` = list filtered to object items (non-list → `[]`).

**NOTE (docs vs code)**: the docs table omits `session.close` permission nuance, documents
`window.input`/`window.form` capability flags; code is authoritative above.

### 2.5 Session JSON (`sessions.list` / `sessions.focused`)

```json
{
  "id": "tab3", "kind": "claude|shell|reader|setup|plugin", "title": "...", "cwd": "...",
  "folder": "<folder root>", "branch": "...", "status": "<ClaudeStatus name>",
  "statusLabel": "<label>", "needsYou": bool, "sessionId": "<resume id or null>",
  "done": bool, "hibernated": bool, "exited": bool, "onScreen": bool, "focused": bool,
  "startedAt": "<ISO-8601 local, no Z>", "project": "<name|null>", "featureOrHotfix": "<name|null>",
  "featureOrHotfixKind": "feature|hotfix|null", "workspaces": ["..."], "launcher": "<name|null>",
  "owner": "<pluginId|null>", "embedded": bool,
  "tag": "<only when owner == caller>",
  "activity": { "prompts": int, "tools": int, "touched": [path], "lastPrompt": str|null, "lastMessage": str|null }
}
```
`activity` only when the caller has `hooks`. `needsYou = !done && status ∈ {waitingInput, waitingAnswer, waitingPermission}`.
ClaudeStatus names/labels: `unknown` `—`, `starting` `Abrindo`, `ready` `Pronto`, `working` `Pensando`,
`tool` `Rodando`, `waitingInput` `Esperando você`, `waitingAnswer` `Pergunta`,
`waitingPermission` `Permissão`, `idle` `Ocioso`, `ended` `Encerrada`.
Non-claude tabs (reader/setup/plugin) report `unknown`.

### 2.6 Events (`event` notification `type`)

| type | fields | activations | needs |
|---|---|---|---|
| `hook` | `tabId`, `name`, `payload` (raw Claude Code hook JSON) | `onHook:<name>`, `onHook:*` | hooks |
| `session.opened` | `tabId`, `kind`, `cwd`, `folder` | `onSession` | — |
| `session.closed` | `tabId` (only for non-passive tabs) | `onSession` | — |
| `session.exited` | `tabId`, `code` (int\|null) | `onSession` | — |
| `session.status` | `tabId`, `status`, `previous` (emitted after a hook changes status) | `onSession` | — |
| `sidebar.shown` | — | n/a: sent via `tell` only if up; otherwise `wake` + sent from `onReady` | — |
| `sidebar.hidden` | — | only if up | — |

Order: for one hook, `hook` is emitted before `session.status`. Events are sent only after
`initialize` returned (deliver awaits start then checks `ready`).

`sidebar.shown` is sent on every click on the plugin's rail icon (even if already shown) and at app
start if the tab was left visible; `sidebar.hidden` when another tab is chosen or the sidebar is hidden.
Only for plugins with `contributes.sidebar`.

### 2.7 Timeouts summary

| what | value |
|---|---|
| `initialize` | 10 s |
| any other host request (`command.invoke`, `suggestion.start`) | 30 s |
| `tool.call` (`TOOL_TIMEOUT`) | 120 s |
| shutdown grace, then SIGTERM grace | 0.8 s + 0.8 s, then SIGKILL |
| plugin → host requests | no host-side timeout (the UI ones wait for the user) |

### 2.8 Restart / crash semantics

- Crash = process exits without host asking. The plugin is shown as `Parou`; views keep their last
  content with a red strip: icon + reason + `Reiniciar` button (or `O plugin não está mais instalado`).
  Floats of crashed/disabled plugins are hidden (not removed); a new `float.show` is needed after restart.
- Revived by: explicit command invocation, `suggestion.start`, `wake` (sidebar tab click),
  `restart`. **Never** by events.
- A rescanned plugin whose id/main didn't change keeps its process.

---

## 3. UI description formats

A view/sidebar/float shows **either** blocks (JSON) **or** an RFW library (text) + data. RFW wins
when `rfwLibrary != null`. Sending `blocks` through `view.update`, `sidebar.update`, `float.show`/
`float.update` clears the library (see §4.2 NOTE for `view.open`).

### 3.1 Blocks (JSON)

Rendering contexts: **framed** pane (padding 22/18/22/32, max width 760, centered, scroll),
**bare** (sidebar/float: padding 14/12/14/20, font −1), **modal** (framed=false, pane metrics).
Base font size `S = markdownBase + tab zoom − (bare ? 1 : 0)`. Blocks are stacked with bottom gap
12 (bare 10); the last child in `card`/`columns` has no gap.

**Expanding layout**: if any *top-level* block is `console` or `terminal` with `expand:true`, the
first such block fills remaining height; blocks before it are fixed on top, blocks after it fixed below
(12 gap); no outer scroll. Padding 14/12/14/12.

Unknown `type` → faint mono text `Bloco desconhecido: <type>` (or `Bloco desconhecido: (sem "type")`).
Blocks needing an `id` without one render red mono `input sem "id"`, `checkbox sem "id"`,
`select sem "id"`, `calendar sem "id"`, `console sem "id"`.

`actionOf(b)` = `b.action ?? b.id` if string (buttons/tabs/sections/list items accept `id` as action).

| type | fields | behavior |
|---|---|---|
| `heading` | `text` | size S+3, w600 |
| `text` | `text`, `style?`, `align?:"center"` | selectable; styles: `dim`, `faint` (S−1), `mono` (S−1.5), `error` red, `success` green, `warning` yellow |
| `markdown` | `text` | GFM, links followed like the reader |
| `code` | `text` | mono S−1.7 in rounded box |
| `divider` | — | 1px border color |
| `section` | `text`, `count?`, `collapsed?` (bool), `action?`, `style?:"label"` | `label`: just the word (S−1.5, w600, faint). Else ruler: [chevron ▸/▾ if `collapsed` is bool] word (10.5, letterSpacing .5) + line + count. Clickable when action. |
| `button` | `action`/`id`, `label?`, `style?` (`primary`,`danger`,`icon`), `icon?`, `tone?`, `tooltip?`, `disabled?`, `menu?` | icon-only when icon known and (`style=="icon"` or no `label`): 30×30, tooltip = tooltip ?? label, color tone or dim. With icon+label: filled (primary) or outlined icon button. Without icon: primary filled, danger red outlined, else outlined. Label default `action ?? 'ok'`. With non-empty `menu`: click opens menu below the button instead of an action. Standalone button is left-aligned. |
| `row` | `children`, `align?` (`center`,`end`,`between`) | wrap, spacing 8; non-button children get width `child.width ?? 240` |
| `input` | `id`, `label?`, `placeholder?`, `value?`, `multiline?` (3–12 lines), `submit?`, `icon?`, `disabled?` | Enter (single-line) sends `submit` action |
| `checkbox` | `id`, `label` (default id), `value?`, `action?`, `disabled?` | toggles local value, then sends action |
| `select` | `id`, `options`, `value?`, `label?`, `placeholder?`, `loading?`, `action?`, `disabled?` | options: string or `{value,label?}`; value `""` valid; value not among options → none selected; `loading` → spinner, disabled; menu max height 320; on change sets value then sends action |
| `list` | `items`, `empty?` (default `Nada aqui`), `flat?`, `alwaysActions?` | see below |
| `kv` | `items:[{key,value,tone?}]` | 2-column table; values selectable; tone colors value |
| `progress` | `value?` (0–1 clamp; absent = indeterminate), `label?`, `detail?`, `tone?` | bar 5px, accent default |
| `console` | `id`, `lines?`, `max?` (5000), `expand?`, `height?` (320), `empty?` (default `Nada no console ainda`), `follow?` (default true) | mono S−2; selectable; follows tail while scrolled within 24px of bottom; new console id → scroll to top. Line tone/spans tones as §3.4 |
| `calendar` | `id`, `mode?` (`single`/`range`), `value?`, `min?`, `max?` (date or `"today"`), `action?`, `presets?:[{label?,from,to?}]` | value `"YYYY-MM-DD"` or `{from,to}`; range: 1st click sets start (value from=to=day), 2nd closes (ordered), 3rd restarts; Portuguese month names, weekday initials `D S T Q Q S S` (Sunday first); summary texts: `Escolha o primeiro dia` / `Escolha um dia` / `Começa em dd/mm — clique no último dia` / `<Weekday>, <d> de <month>` / `dd/mm a dd/mm · <n> dias`; presets side column when width ≥ 470 else wrap row; centered |
| `header` | `title`, `subtitle?`, `tone?` (subtitle color), `avatar?`, `status?`, `dim?`, `wrap?` (≤3 lines), `actions?:[button]` | avatar 40 (bare 30); title S+5 (bare S+2.5) w700; each action rendered as button with `style:"icon"` |
| `card` | `children`, `padding?` (default 12, bare 6) | surface = fg at 3.5 % (dark) / 2.5 % (light) over current surface; radius `Mx.radius`; border |
| `tabs` | `items:[{label, action/id, active?, count?, icon?}]`, `align?:"center"` | segmented control; active chip raised |
| `columns` | `children` | equal-width columns, gap 14 |
| `terminal` | `tabId`, `expand?`, `height?` (360), `autofocus?`, `empty?` (default `O terminal fechou`) | only tabs owned by this plugin and `embedded`; otherwise shows `empty` |

**List item** fields: `title`, `subtitle?`, `icon?`, `tone?` (icon color), `badge?`, `action`/`id`,
`actions?:[{action, icon, tooltip?, tone?}]` (items without a known icon dropped; shown on hover, or
always with `alwaysActions`; replace `meta`), `menu?` (right-click), `avatar?`, `status?` (tone dot),
`dim?`, `strong?`, `indent?` (×16 flat / ×18 framed), `expanded?` (bool → chevron; clicking it sends
`toggle ?? action`; rows without a chevron in a list where any has one get a 21px spacer), `meta?`,
`metaTone?`, `subtitleTone?`, `selected?` (accent background, text on-accent).
Framed list: bordered box with dividers; flat: no border, rounded hover, single-line ellipsized
titles. Avatar size 26 flat / 30 framed. Clicking an action button sends only that action.

**Menu** (list item `menu`, button `menu`): `[{action, label?, icon?, tone?, disabled?, children?} | {type:"divider"}]`;
`children:[{action, label?, tone?}]` → submenu with color dots (tone `""`/absent → outline dot).
Choosing sends `view.action` with that action.

**Avatar** (`avatar`): string icon name/svg path, or `{icon?, text?, color? (tone), shape?:"circle", plain?, svg?}`.
`svg` → raw SVG drawn with its own colors (dim → 70 % opacity). `plain` → glyph only (80 % size).
Else rounded square (radius 28 % of size, or circle): fill color at 17 % (dim 10 %), border 26 %
(dim 14 %), content `text` (40 % size, w700) or glyph (56 %). `dim` uses faint color. `status` →
dot (9px if size < 32 else 11) bottom-right with 2px ring of the surface color.

### 3.2 Field state & `values`

Per view (per rendered pane): text controllers by input id; picks by id for checkbox/select/calendar.
On every render (`_sync`), walking top level and `row`/`card`/`columns` children:
- `input`: create with `value` (string or `""`); if the plugin sent a **different** `value` than last
  time, overwrite the text; otherwise keep what the user typed.
- `checkbox`/`select`/`calendar`: set pick when absent or when the sent `value` differs **by deep
  equality** from the last sent value.
- Ids no longer present are dropped.
`values` = `{inputId: text, ...picks}` (picks overwrite text ids on collision). Every action from a
blocks view sends the full `values`.

### 3.3 Icons and tones

Icon names (`pluginIcon`): check, error, warning, info, file, folder, play, terminal, link, star,
clock, user, task, continue, pause, step-over, step-into, step-out, reload, restart, stop, devtools,
bug, device, refresh, clear, copy, search, settings, calendar, report, receipt, open, add, remove,
undo, branch, commit, push, pull, sync, server, edit, dot, box, stack, image, drive, network, cpu,
trash, globe, more, **chevron, filter, layers** (last three undocumented). Unknown → generic
puzzle icon (`extension_outlined`) in glyphs; `null` (not shown) where noted.
Material mapping (for picking equivalent Rust icons): check→check_circle_outline,
error→error_outline, warning→warning_amber_outlined, info→info_outline, file→description_outlined,
folder→folder_outlined, play→play_arrow, terminal→terminal, link→link, star→star_outline,
clock→schedule, user→person_outline, task→task_alt, continue→play_arrow_rounded,
pause→pause_rounded, step-over→redo_rounded, step-into→south_rounded, step-out→north_rounded,
reload→bolt_rounded, restart→replay_rounded, stop→stop_rounded, devtools→travel_explore_rounded,
bug→bug_report_outlined, device→smartphone_outlined, refresh→refresh_rounded, clear→block_rounded,
copy→copy_rounded, search→search_rounded, settings→settings_outlined, calendar→calendar_today_outlined,
report→summarize_outlined, receipt→receipt_long_outlined, open→open_in_new_rounded, add→add_rounded,
remove→remove_rounded, undo→undo_rounded, branch→alt_route_rounded, commit→commit_rounded,
push→upload_rounded, pull→download_rounded, sync→sync_rounded, server→dns_outlined,
edit→edit_outlined, dot→circle, box→inventory_2_outlined, stack→layers_outlined, image→album_outlined,
drive→storage_rounded, network→lan_outlined, cpu→memory_rounded, trash→delete_outline_rounded,
globe→public_rounded, more→more_horiz_rounded, chevron→chevron_right_rounded,
filter→filter_list_rounded, layers→layers_outlined.

`PluginGlyph(icon, dir)`: if icon ends with `.svg` (case-insens.), contains no `..`, resolves inside
the plugin dir and exists → SVG file tinted with a single color (srcIn); else named icon.

Tone → color: `#hex` (parseHexColor, invalid → fgDim); `green`/`success`; `red`/`error`;
`yellow`/`warning`; `accent`; `purple`; `faint`; `blue`/`cyan`/`magenta` (ANSI palette);
`white` → fg; `black` → fgFaint; anything else (incl. null where used as fallback) → fgDim.

### 3.4 RFW (Remote Flutter Widgets 1.1.4) text format — grammar, runtime, widget catalog

Plugins send `rfw: { library: <text>, root?: <widget name> }` plus `data`. Every real plugin reads the
library from a `.rfwtxt` file in its folder; none builds library text in code. Root names used: `root`
(default) everywhere, and `home`/`tree` (argocd, same library, two roots). A Rust host cannot run the
Dart rfw package, so it must implement: (a) the text parser, (b) the evaluator (references, loops,
switches, events), (c) the local widget subset below, with rfw's exact argument decoding. A golden
test is to parse all 7 real files: `docker/ui/sidebar.rfwtxt`, `ssh/ui/hosts.rfwtxt`,
`pomodoro/ui/{float,sidebar}.rfwtxt` (pomodoro_dart copies are identical), `argocd/ui/argo.rfwtxt`,
`wiboor/ui/kanban.rfwtxt`, `flutter/ui/devtools.rfwtxt`.

Host wiring (`plugin_rfw.dart`): libraries `core.widgets`, `core.material`, `maestria` (local) and
`plugin` (the parsed text); root widget = `plugin:<rfwRoot>`; the tree is wrapped in a default text
style (color fg, size 13). Library text is re-parsed only when it changes; on parse error the surface
shows red mono text `a interface do plugin não leu:\n<message> at line L column C.` (11.5px, padding 14).
No library yet → nothing rendered. Events → `view.action {viewId, action: <event name>, values: <args>}`.

#### A. Grammar (text.dart: `parseLibraryFile`, `_tokenize`, `_Parser`)

##### A.1 Lexical rules (exact tokenizer behaviour)

The input is iterated as Unicode code points (`file.runes`). Line and column are 1-based. Columns count code points (the docs say UTF-16 units).

**Whitespace.** Only U+0020 (space) and U+000A (LF) are whitespace.
- **TAB (U+0009) and CR (U+000D) outside strings are a hard error** ("Unexpected character U+0009"). A CRLF file fails to parse.
- The real files contain no tabs, no CR and no block comments.

**Symbols** (yielded from "main" mode): `( ) , : ; = > [ ] { }`.
- `.` becomes the DOT token.
- `...` (three dots) becomes the TRIPLEDOT token.
- `>` is only accepted in main mode, so it must follow whitespace or another symbol. `=>` works because `=` returns the tokenizer to main mode.

**Comments.**
- `//` runs to LF or EOF.
- `/* ... */` does not nest; EOF inside one is an error.
- `/` followed by anything other than `/` or `*` is an error.
- **Gotcha:** a `/` is only recognised in main mode. Immediately after an identifier, a number or a closing quote it is an error. `foo// x` and `"a"//x` fail; `foo // x` is fine. The same applies to `-`, `"` or `>` glued to an identifier or number.

**Identifiers.** `[A-Za-z_][A-Za-z0-9_]*`. An identifier ends on space, LF, any symbol except `>`, or `.`, which switches to "dot" mode.

**Strings.** `"..."` or `'...'`.
- A raw LF inside a string is an error. Any other code point, including TAB and non-ASCII, is taken literally.
- Escapes: `\" \' \\ \/ \b \f \n \r \t \uXXXX`. `\u` takes exactly 4 hex digits, which are added as one code unit; a surrogate pair needs two escapes.
- Any other escape is an error.
- After the closing quote the next character must be space, LF, a symbol (not `>`) or `.`, which continues a reference part.
- The real files use only `"..."`, with no escapes; there is non-ASCII in strings, e.g. `"›"`, `"próprio"`.

**Integers.**
- Decimal: `-?[0-9]+`. Leading zeros are allowed (`007` is 7) and `-0` is 0.
- Hex: `0x` or `0X` followed by `[0-9A-Fa-f]+`. Hex is always positive; `-0x..` is an error.
- Values are 64-bit signed.

**Doubles.** `-?[0-9]+(\.[0-9]+)?([eE]-?[0-9]+)?`, where at least a fraction or an exponent makes it a double. `1e3` is a DOUBLE.
- Errors: `1.`, a bare `.5` (tokenizes as DOT then INT, then fails in the parser), `+1`, and `e+3` (a plus sign in the exponent).

**Numbers inside references.** After `.` (dot mode), a digit starts "integerOnly" mode. In that mode a following `.` ends the integer and starts a new part. So `data.rows.0.name` tokenizes as `data . rows . 0 . name`.

**Booleans and null.** `true`, `false` and `null` are plain identifiers, interpreted by the parser.

**Reserved words** (checked only for loop variable names and builder argument names): `args data event false set state true`.
- `switch`, `default`, `null`, `for`, `in`, `import` and `widget` are not reserved, but they are context keywords.

##### A.2 EBNF (library file)

```ebnf
libraryFile   ::= import* widgetDecl* EOF
import        ::= "import" key ("." key)* ";"            (* key = IDENT | STRING *)
widgetDecl    ::= "widget" IDENT initialState? "=" (constructorCall | switch) ";"
initialState  ::= dataMap                                 (* parsed with extended=false *)

constructorCall ::= IDENT "(" argEntries ")"              (* IDENT must not be a reserved/handled word, see resolution order *)
argEntries    ::= ( entry ("," entry)* ","? )?             (* same as map body *)
entry         ::= key ":" (value | "null")                (* "null" => key omitted; duplicate keys = error *)
key           ::= IDENT | STRING                          (* reserved words ARE allowed as keys *)

value         ::= map | list | INT | DOUBLE | STRING | "true" | "false"
                | eventHandler | setStateHandler
                | argsRef | dataRef | stateRef | loopRef | builderArgRef
                | switch | widgetBuilder | constructorCall
map           ::= "{" argEntries "}"
list          ::= "[" ( listItem ("," listItem)* ","? )? "]"   (* commas mandatory between items *)
listItem      ::= value | loop
loop          ::= "..." "for" IDENT "in" value ":" value        (* only inside lists (extended) *)

argsRef       ::= "args"  parts
dataRef       ::= "data"  parts
stateRef      ::= "state" parts
loopRef       ::= LOOPIDENT partsOpt                    (* parts optional: bare `row` allowed *)
builderArgRef ::= BUILDERIDENT parts                    (* NOTE: parts mandatory here *)
parts         ::= ("." (IDENT | STRING | INT))+          (* required for args/data/state *)
partsOpt      ::= ("." (IDENT | STRING | INT))*

eventHandler  ::= "event" STRING map                     (* map is mandatory, may be {} *)
setStateHandler ::= "set" "state" parts "=" value

switch        ::= "switch" value "{" ( case ("," case)* ","? )? "}"
case          ::= ("default" | value) ":" value          (* case key may not start with a symbol token; at most one default; duplicate keys = error *)

widgetBuilder ::= "(" IDENT ")" "=" ">" (constructorCall | switch)   (* exists in 1.1.4 *)

(* data-file / initialState mode (extended=false): only map, list, INT, DOUBLE, STRING, true, false, and "null" as a map value *)
```

**How an identifier in value position is resolved** (`_readValue`, in this order):
1. `true` and `false`.
2. `null`, only where a map value is allowed (it yields `missing`, so the key is dropped). Anywhere else, `null` falls through to step 9 and fails with "expected (".
3. If not in extended mode, any other identifier is an error.
4. `event` → EventHandler.
5. `args`, `data` or `state` → the matching reference.
6. `switch` → Switch.
7. `set` → SetStateHandler.
8. A name in the widget-builder scope, then a loop variable. For loop variables, `lastIndexOf` finds the innermost one, producing `LoopReference(depth, parts)`, where depth is a De Bruijn index and the innermost loop is 0.
9. Otherwise the identifier is parsed as a constructor call, `Name(...)`.

**A note on `widget` declarations.**
- The root must be a ConstructorCall or a `switch`. `widget A = args.child;` is invalid.
- The initial state only allows literal data.

##### A.3 Semantics

**References.**
- `args.*` indexes the argument map that the caller passed to the remote widget currently being defined. Arguments are bound lazily, as unevaluated nodes; see B.2.
- `data.*` indexes the `DynamicContent`. It is live and subscribed.
- `state.*` indexes the nearest enclosing stateful remote widget's state, by depth.
- A loop variable is the list element. Builder arguments are the map passed to the builder.
- String or identifier parts index maps; integer parts index lists. Any mismatch, missing key or out-of-range index gives the sentinel `missing`, which every typed getter reads as null.
- Indexing into a scalar (`data.x.y` where `x` is an int) gives `missing` in the runtime resolver. In `DynamicContent` subscriptions, `_DynamicNode._prepare` hands back the parent scalar itself. This is a quirk and is not relevant to the real files.

**Switch.**
- The control value is fully resolved, with no parts.
- It is looked up in a Dart `Map<Object?,Object>` of case keys. Dart `==`/`hashCode` are used, so:
  - strings, bools and numbers match by value;
  - **int and double compare numerically equal, so case `1` matches 1.0, and `0.0` matches int 0**;
  - maps and lists never match, because Dart equality on them is identity.
- No match, or a `missing` control value, falls back to the `default` case. With no default, the switch evaluates to `missing`.
- The chosen branch is evaluated lazily, and any remaining reference parts continue indexing into it.
- A widget-root switch (`widget X = switch ...`) that does not resolve to a widget renders an error widget: "Switch in plugin:X did not resolve to a widget (got ...)".

**Loops.**
- `...for x in L: T` is valid only inside a list.
- When the list is consumed, the loop expands in place: each element of the resolved `L` produces one instance of `T`, with `x` bound to that element.
- `L` can be any value that resolves to a list: an args, data or state ref, another loop variable, or a switch. kanban uses `...for w in switch data.statsSkeleton { true: [78.0,112.0,96.0], default: [] }:`.
- If `L` resolves to a non-list (a map, a scalar or `missing`), it expands to zero items.
- Nested loops bind depth+1 inside the inner template. Effective list indices count expanded items (`_listLookup`).

**Events.**
- `event "name" { k: v, ... }`: when the handler is fetched, its argument map is deep-resolved (`_fix`: references, switches and loops inside lists are all evaluated).
- On trigger, `remoteEventTarget(name, args)` is called.
- If the widget passes extra arguments (e.g. Pressable's `onMenu` gives `{action: choice}`; DropTarget gives `{payload, to}`), they are appended after the declared entries. On a duplicate key the extra value wins (`DynamicMap.fromEntries(args.entries.followedBy(extra.entries))`).
- A handler argument may also be a list of handlers; all of them fire in order. This is not used.
- In the host, events become the JSON-RPC notification `view.action` with params `{viewId, action: <eventName>, values: <args as plain JSON>}`.
- A `missing` value inside the event arguments is left as the `Missing` object by the host's `_plain`, which would break JSON encoding; the Rust side should drop such keys. None of the real files hit this as long as the data has the fields.

**set state.**
- `set state.a.b = value`: the value is resolved when fetched, and the path must exist in the state.
- Assigning to a map key that does not exist, or to a list index out of range, throws `RemoteFlutterWidgetsException("... does not identify existing state.")`.
- After assigning, the state DynamicContent is updated (`updateAll`) and subscribers rebuild. The state depth is bound when the widget is curried.

**Widget builders.** `(scope) => Widget(...)` exists in 1.1.4 (`WidgetBuilderDeclaration`). It is called by `source.builder(...)` or `optionalBuilder(...)` with a `DynamicMap`. It is lexically scoped.

**Constructs actually used by the real files:**
- `import`.
- Stateless `widget X = Call(...)` and `widget X = switch ... {...}`.
- Constructor calls; maps; lists; trailing commas; `//` comments.
- `...for` loops, including a loop over a switch.
- `args.*`, `data.*`, bare loop variables (`row`, `t`, `d`, `h`, `w`) and loop variable parts (`row.kind`).
- `switch` with string, bool, int (`indent { 1: ... }`) and double (`switch args.line { 0.0: ... }`) case keys, always with a `default`.
- `event "act" {...}`, `event "act" {}`, `event "drop" {}`.
- Hex ints (`0x00000000`, `0xFFFFFFFF`); negative doubles (`-1.0`, `-84.0`); ints (`maxLines: 1`, `duration: 140`, `flex: 3`).

**Never used:** state declarations, `state.*`, `set state`, widget builders, `null`, single-quoted strings, escapes, block comments, string or int parts in references (`data.rows.0`, `data."k"`), exponent numbers, outer-loop references from inside nested loops.

---

#### B. Runtime semantics

##### B.1 Libraries and lookup

The host registers:
- `core.widgets` = `createCoreWidgets()`
- `core.material` = `createMaterialWidgets()`
- `maestria` = the local widgets
- `plugin` = `parseLibraryFile(text)`

The root is `FullyQualifiedWidgetName(plugin, rfwRoot)`.

**Name lookup** (`_findConstructor`):
1. The library's own declarations, first match wins.
2. Then each import in declaration order, recursively (depth-first).
3. A local library is a map lookup.

Import loops throw.

Name clashes are harmless here:
- wiboor and devtools declare a remote `Card` and do not import `core.material`, which also defines `Card`.
- pomodoro imports `core.material`, but none of its remote names clash.
- `IconButton` and `Tooltip` are not in `core.widgets` or `core.material` 1.1.4, so they resolve to `maestria`.

**Unknown widget name.** This is not a parse error. At render the runtime produces `ErrorWidget` with "Could not find remote widget named X in plugin." If an imported library is missing, the message lists the missing dependencies.

**Recursive widget.** "Widget loop: Tried to call ... reentrantly" renders as an error widget.

**Parse error.** The host shows a `SelectableText` with `'a interface do plugin não leu:\n$e'`, where `$e` is `"<message> at line L column C."`, in mono, size 11.5, red, inside padding 14.

##### B.2 Binding (currying) and resolution

When a remote widget is instantiated, its arguments are not evaluated.
- `args.*` inside its body becomes `BoundArgsReference(callerArgsMap, parts)`, a thunk.
- Data references stay as `DataReference` and are resolved live.
- The result is a `_CurriedWidget` tree: local, remote with state, or switch.

When a local widget asks `source.v(...)` and friends for a key path, `_resolveFrom` walks:
- `DataReference` → `dataResolver(parts + rest)`, which subscribes.
- `BoundArgsReference` → continue in the bound map with the concatenated parts.
- `BoundLoopReference` → continue in the bound value.
- `BoundStateReference` → `stateResolver`.
- `Switch` → choose a branch, then continue.
- `DynamicMap[String]`, `DynamicList[int]`. List indexing counts loop expansions.
- Anything else → `missing`.

At the end of the walk, an EventHandler or SetStateHandler gets its arguments `_fix`-resolved.

Results are cached per argument path and invalidated through the data subscriptions.

##### B.3 DataSource API (what local builders use)

**`v<T>(path)`** for `T` in int, double, bool, String: returns the value only if `value is T`, otherwise null.
- **Strict typing:** an int where a double is expected gives null, and a double where an int is expected gives null.
- Host JSON comes from Dart `jsonDecode`, so a JSON number without `.` or an exponent is an int. JS `JSON.stringify(12.0)` emits `12`, so it arrives as an int.
- The plugins work around this deliberately. pomodoro clamps `progress` to [0.0001, 0.9999]. argocd uses `w: 1.5, h: 1.5` placeholders. wiboor uses `frac(...)` for `bar.value`. devtools uses int `flex` because it is read as int.
- In the Rust implementation, keep the int/double distinction from JSON and from the rfwtxt literals: `12` is int, `12.0` is double.

**`isList(path)`**: true for a list, including one with loops. **`isMap(path)`**: true for a map.

**`length(path)`**: the length of the list after loop expansion. Anything that is not a list gives 0.

**`child(path)`**: if the value is a widget, builds it. Otherwise it renders an ErrorWidget: "Not a widget at [path] (got X) for plugin:Name."

**`optionalChild(path)`**: the widget, or null.

**`childList(path)`**:
- A list (loops expanded) gives one entry per element; an element that is not a widget becomes an ErrorWidget in its place.
- `missing` gives an empty list.
- Any other value gives `[ErrorWidget("Not a widget list ...")]`.

**`ListView` and `GridView`** do not use `childList`. They use `length(['children'])` together with `child(['children', i])`.

**`voidHandler(path, [extra])`** and **`handler(path, generator)`**:
- The value is fetched with `expandLists: true`.
- A single `AnyEventHandler` is wrapped into a list. Otherwise the expanded list is filtered to its handlers.
- If there are no handlers, the result is null. The widget is then disabled; for example IconButton draws at 0.4 alpha.
- Otherwise the callback fires each handler in order: EventHandler goes to the host with merged extra arguments, SetStateHandler applies the state change.
- `voidHandler` passes the `extraArguments` given at fetch time; `handler` lets the widget pass a map at trigger time.

**`builder` / `optionalBuilder`**: the value must be a builder closure. Not used.

##### B.4 Colors

`ArgumentDecoders.color(source, key)` is `Color(v<int>(key))`, a 32-bit ARGB value `0xAARRGGBB`. A double or a string gives null.
- Literals in the files: `0x00000000` (transparent) and `0xFFFFFFFF`.
- `data.theme.*` values are ints built with `Color.toARGB32()` in the host.
- Plugins build `0xff000000 + parseInt(hex, 16)` (pomodoro `argb`, docker menu children colors).

##### B.5 DynamicContent

- It is a tree rooted at a map.
- `update(rootKey, value)` deep-clones the value and replaces that top-level key.
  - If the old value `==` the new one, nothing happens. Maps and lists compare by identity, so they are always replaced.
  - Otherwise the node's existing child subscriptions are updated recursively: keys that vanished become `missing`, a primitive that changed notifies its subscribers.
- `updateAll(map)` calls `update` per key; a null value becomes `missing`.
- **Host:**
  - `data.theme` is set by the host from the palette and re-pushed when the theme changes. The plugin cannot override `theme`; that key is skipped.
  - On each new `rfwRevision`, every entry of `rfwData` is pushed with `_content.update(key, value)`. `rfwData` is the merge (`addAll`) of every `data` the plugin sent, so updates are per top-level key and keys are never deleted.
  - The library text is re-parsed only when it changes.

**Theme keys:** `dark` (bool), `canvas`, `bg`, `sidebar`, `hover`, `active`, `border`, `fg`, `dim`, `faint`, `accent`, `onAccent` (either 0xFFFFFFFF or canvas, depending on the accent's brightness), `green`, `yellow`, `red`, `purple`, `blue`, `cyan`, `magenta` (all ARGB ints), and `mono` (String, the font family).

**Host wrapper:** `DefaultTextStyle.merge(style: TextStyle(color: Mx.fg, fontSize: 13))` around the RemoteWidget.

##### B.6 Animation defaults

Several core widgets are the Animated* variants: Align, Container, Opacity, Padding, Positioned and Rotation, plus DefaultTextStyle, which no file uses. When a property changes they animate with:
- `duration`: `v<int>` milliseconds, default 200 ms (`AnimationDefaults.durationOf`);
- `curve`: a string name, default `Curves.fastOutSlowIn`.

docker passes `duration: 140` to `Rotation`.

##### B.7 Missing arguments and error rendering

- Missing optional arguments fall back to the defaults listed in section C.
- A missing *required* child (`source.child`) renders a Flutter `ErrorWidget` (red box with message) and reports through `FlutterError.reportError`. In this set that applies to Expanded, Flexible, FractionallySizedBox and Positioned (plus Tooltip in `maestria`).
- `Text` with a missing or non-string `text` renders `""`. If `text` is a list, its string elements are concatenated and non-strings become `""`.

---

#### C. Catalog of widgets used, with the exact arguments read

Notation: `name: decoder = default`. "USED" lists what the real files pass.

##### C.1 `core.widgets` (`core_widgets.dart`)

**Align** (AnimatedAlign)
- Reads: `duration`, `curve`; `alignment: alignment = Alignment.center`; `widthFactor: v<double>`; `heightFactor: v<double>`; `onEnd: voidHandler`; `child: optionalChild`.
- USED: `alignment` (a map `{x,y}` or a switch of maps), `child`.

**Center**
- Reads: `widthFactor: v<double>`, `heightFactor: v<double>`, `child: optionalChild`.
- USED: `child`.

**Column** and **Row** (identical readers)
- `mainAxisAlignment: enum MainAxisAlignment = start`. Values: `start end center spaceBetween spaceAround spaceEvenly`.
- `mainAxisSize: enum MainAxisSize = max`. Values: `min max`.
- `crossAxisAlignment: enum CrossAxisAlignment = center`. Values: `start end center stretch baseline`.
- `textDirection: enum` (`rtl`, `ltr`); `verticalDirection: enum = down` (`up`, `down`); `textBaseline: enum` (`alphabetic`, `ideographic`).
- `children: childList`.
- USED: `children`, `crossAxisAlignment` (`"stretch"`×34, `"start"`×22, `"center"`×4 counting Wrap, `"end"`×1), `mainAxisAlignment` (`"center"`), `mainAxisSize` (`"min"`).

**Container** (AnimatedContainer)
- Reads: `duration`, `curve`; `alignment: alignment`; `padding: edgeInsets`; `color: color`; `decoration: decoration`; `foregroundDecoration: decoration`; `width: v<double>`; `height: v<double>`; `constraints: boxConstraints`; `margin: edgeInsets`; `transform: matrix`; `transformAlignment: alignment`; `clipBehavior: enum Clip = none` (`none hardEdge antiAlias antiAliasWithSaveLayer`); `onEnd`; `child: optionalChild`.
- Flutter asserts if both `color` and `decoration` are given. No real file does that.
- USED: `width`, `height` (double literals or `args.size`/`args.width`/`args.height`), `padding`, `margin`, `color` (ref or switch), `decoration` (box), `clipBehavior: "antiAlias"`, `alignment` (devtools Toggle), `child` (including `args.child` and switch).

**Expanded**
- Reads: `flex: v<int> = 1`, `child: child` (required).
- USED: `child`, `flex` (an int literal or an int from data such as `args.top` and `args.s.used`).

**Flexible**
- Reads: `flex: v<int> = 1`, `fit: enum FlexFit = loose` (`tight`, `loose`), `child: child` (required).
- USED: `child`.

**FractionallySizedBox**
- Reads: `alignment = center`, `widthFactor: v<double>`, `heightFactor: v<double>`, `child: child` (required).
- USED: `widthFactor` (from args, a double), `heightFactor: 1.0`, `child`.

**GestureDetector**
- Reads: `onTap`, `onTapCancel`, `onDoubleTap`, `onLongPress` (voidHandler); `onTapDown`, `onTapUp` (handler, details ignored); `behavior: enum HitTestBehavior` (`deferToChild opaque translucent`); `child: optionalChild`.
- USED: `onTap: event`, `behavior: "opaque"`, `child` (devtools FrameBar).

**GridView** (GridView.builder)
- Reads: `scrollDirection: enum Axis = vertical` (`horizontal`, `vertical`); `reverse: v<bool> = false`; `primary: v<bool>`; `shrinkWrap: v<bool> = false`; `padding: edgeInsets`; `gridDelegate: gridDelegate = FixedCrossAxisCount(2)`; children via `length` and `child(['children', i])`; `addAutomaticKeepAlives`, `addRepaintBoundaries`, `addSemanticIndexes` (bool, default true); `cacheExtent: double`; `semanticChildCount: int`; `dragStartBehavior`; `keyboardDismissBehavior`; `restorationId`; `clipBehavior = hardEdge`.
- USED (ssh only): `shrinkWrap: true`, `primary: false`, `gridDelegate: {type:"maxCrossAxisExtent", maxCrossAxisExtent:320.0, mainAxisExtent:58.0, mainAxisSpacing:10.0, crossAxisSpacing:10.0}`, and `children: [...for t in args.s.items: Tile(t: t)]`.

**IntrinsicHeight**
- Reads: `child: optionalChild`.
- USED: `child` (wiboor Card).

**ListView** (ListView.builder)
- Reads: `scrollDirection = vertical`; `reverse = false`; `primary`; `shrinkWrap = false`; `padding: edgeInsets`; `itemExtent: v<double>`; `prototypeItem: optionalChild`; `itemCount = length(children)`; item `i` is `child(['children', i])`; `clipBehavior = hardEdge`; the same keep-alive, cache and drag flags as GridView.
- USED: `padding`, `children`; `scrollDirection: "horizontal"`, `reverse: true`, `itemExtent: 9.0` (devtools FrameChart).

**Opacity** (AnimatedOpacity)
- Reads: `opacity: v<double> = 0.0`. **The default is 0, so a missing value makes the child invisible.** Also `alwaysIncludeSemantics = true`, `onEnd`, `child: optionalChild`.
- USED: `opacity: switch args.dim { true: 0.72, default: 1.0 }`, `child` (docker).

**Padding** (AnimatedPadding)
- Reads: `padding: edgeInsets = EdgeInsets.zero`, `onEnd`, `child: optionalChild`.
- USED: `padding` (a list or a switch of lists), `child`.

**Positioned** (AnimatedPositionedDirectional)
- Reads **`start`, `top`, `end`, `bottom`**, `width`, `height` (all `v<double>`), `onEnd`, `child: child` (required).
- **`left` and `right` are NOT read.** ssh `hosts.rfwtxt` Avatar passes `right: 0.0, bottom: 0.0`, so in real rfw the `right` is silently ignored. The badge sits at the bottom-start corner (Stack alignment topStart, only `bottom` applied). This is effectively a plugin bug; the renderer should mimic it, or the team should decide whether to.
- With LTR text, `start` is left and `end` is right.
- USED: `start` (including `n.x` and `-84.0`), `top` (including `n.y`), `end`, `bottom`, `width`, `height`, `right` (ignored), `child`.

**Rotation** (AnimatedRotation)
- Reads: `turns: v<double> = 0.0`; `alignment: alignment = center` (resolved with Directionality); `filterQuality`; `duration`, `curve`, `onEnd`; `child: optionalChild`.
- Paint-only: layout is unaffected, as kanban's comment notes.
- USED: `turns` (a double or a switch), `duration: 140`, `child`.

**SingleChildScrollView**
- Reads: `scrollDirection = vertical`; `reverse = false`; `padding: edgeInsets`; **`primary: v<bool> = true`**; `dragStartBehavior`; `clipBehavior = hardEdge`; `restorationId`; `keyboardDismissBehavior`; `child: optionalChild`.
- USED: `primary: false`, `scrollDirection: "horizontal"`, `padding`, `child`.

**SizedBox**
- Reads: `width: v<double>`, `height: v<double>`, `child: optionalChild`.
- USED: `width` and `height` (literals, refs or switches), `child` (including `args.child`). `SizedBox()` with no arguments is common.

**Spacer**
- Reads: `flex: v<int> = 1`.
- USED: no arguments.

**Stack**
- Reads: `alignment: alignment = AlignmentDirectional.topStart`; `textDirection`; `fit: enum StackFit = loose` (`loose expand passthrough`); `clipBehavior = hardEdge`; `children: childList`.
- USED: `children`, `fit: "expand"`.

**Text**
- Reads:
  - `text`: `v<String>`, or a list of strings concatenated, otherwise `""`.
  - `style: textStyle`; `strutStyle`.
  - `textAlign: enum TextAlign` (`left right center justify start end`).
  - `textDirection`; `locale` (a `"ll-CC"` string).
  - `softWrap: v<bool>`.
  - `overflow: enum TextOverflow` (`clip fade ellipsis visible`).
  - `textScaleFactor: v<double>`; `maxLines: v<int>`; `semanticsLabel`; `textWidthBasis`; `textHeightBehavior`.
- USED: `text` (a ref or a string literal), `maxLines` (1, 2, 4), `overflow: "ellipsis"`, `textAlign` (`"center"`, `"end"`), `style` (keys below).

**Wrap**
- Reads:
  - `direction: enum Axis = horizontal`.
  - `alignment: enum WrapAlignment = start` (`start end center spaceBetween spaceAround spaceEvenly`).
  - `spacing: v<double> = 0`; `runAlignment = start`; `runSpacing: v<double> = 0`.
  - `crossAxisAlignment: enum WrapCrossAlignment = start` (`start end center`).
  - `textDirection`; `verticalDirection = down`; `clipBehavior = none`; `children: childList`.
- USED: `spacing`, `runSpacing`, `crossAxisAlignment: "center"`, `children`.

##### C.2 `core.material`

**CircularProgressIndicator**
- Reads: `value: v<double>` (**null means indeterminate spinning**); `color: color`; `backgroundColor: color`; `strokeWidth: v<double> = 4.0`; `semanticsLabel`; `semanticsValue`.
- USED (pomodoro float and sidebar): `value: data.progress`, `strokeWidth` (3.5 or 6.0), `color: data.color`, `backgroundColor: data.theme.active`.

##### C.3 `maestria` (from `plugin_rfw.dart`)

**Svg**
- Reads: `svg: v<String>` (null renders `SizedBox.shrink`), `width: v<double>`, `height: v<double>`.
- Renders `SvgPicture.string` in the SVG's own colors.
- USED: `svg`, `width`, `height` (docker Avatar; argo tree uses `data.graph.*`).

**Glyph**
- Reads: `icon: v<String>`, `size: v<double> = 16`, `color: color` (null means `Mx.fgDim`).
- Icon resolution:
  1. If `icon` ends with `.svg` (case-insensitive), is inside the plugin dir (no `..`) and the file exists: draw the SVG file at size×size, tinted with `ColorFilter.mode(tint, srcIn)`.
  2. Otherwise, a name in the `pluginIcon` map (material icon).
  3. Otherwise `Icons.extension_outlined`.
- **`pluginIcon` names** (in `plugin_pane.dart`): check error warning info file folder play terminal link star clock user task continue pause step-over step-into step-out reload restart stop devtools bug device refresh clear copy search settings calendar report receipt open add remove undo branch commit push pull sync server edit dot box stack image drive network cpu trash globe more chevron filter layers.
- USED: `icon` (names or `"icons/argo.svg"`, `"icons/wiboor.svg"`, `"icons/comment.svg"`, `"icons/attach.svg"`, `"icons/close.svg"`), `size`, `color`.

**IconButton**
- Reads: `disabled: v<bool>` (if true, `onPressed` is null); `onPressed: voidHandler`; `color: color = Mx.fgDim` (alpha 0.4 when disabled or with no handler); `tooltip: v<String>`; `size: v<double> = 16` (icon size); `extent: v<double> = 26` (a square tight box); `icon: v<String>` → `pluginIcon`, or `Icons.circle_outlined`. Padding is zero.
- **SVG paths are not supported here.**
- USED: `icon`, `tooltip`, `onPressed: event "act" {a: ...}`, `color`, `size`, `extent`, `disabled`.

**Tooltip**
- Reads: `message: v<String> = ''`, `child: child` (required).
- USED: `message` (a ref or literal), `child`.

**Pressable**
- Reads (`_read`, `button: false`):
  - `onTap`, `onDoubleTap`: voidHandler.
  - `color`, `hoverColor`: color.
  - `radius: v<double> = 0`, applied as `BorderRadius.circular` only if > 0.
  - `padding: edgeInsets`.
  - `child: optionalChild`.
  - `menu`: list, read by index; see below.
  - `onMenu: handler` (a trigger called with an extra map).
  - `icon`, `tooltip`, `size` are also read but unused when `button` is false.
- Render: a `MouseRegion`, with the click cursor if `onTap` is set. Hover is tracked; the background is `hoverColor` while hovered (if set), otherwise `color`. A `GestureDetector` (opaque) handles `onTap`/`onDoubleTap`; a secondary tap (right click) opens the context menu if it is non-empty. The child sits in a Container with `padding` and the decoration color and radius.
- **Menu entry `i`:**
  - `action`: null if `divider == true`, otherwise `v<String>(['menu', i, 'action'])`. **An entry whose action is null (divider, or no `action` string) renders as a divider.**
  - `label: v<String> = ''`. If empty, the label shown is `action`.
  - `icon: v<String>` (`pluginIcon`, at 14 px).
  - `red: v<bool>` (red text and icon).
  - `disabled: v<bool>`.
  - `children`: a submenu of `{label, action, color: v<int>}`, where color 0 means no dot.
- **Choosing an item** calls `onMenu({action: choice})`, which merges into the declared event arguments. All real files use `onMenu: event "act" {}`, so the plugin receives `{action: "<choice>"}`.
- USED: `radius`, `color`, `hoverColor`, `padding`, `onTap: event`, `menu` (a data or args ref), `onMenu: event "act" {}`, `child`. `onDoubleTap` is never used.

**MenuButton** (Pressable with `button: true`)
- Reads: `icon` (default `more_horiz_rounded`), `tooltip`, `size = 16` (icon size), `color` (default `fgDim`), `menu`, `onMenu`.
- Renders a 26×26 icon button. On click, it opens the menu at the button-local offset `(width - 200, height + 4)`.
- USED: `icon` (`"filter"`, `"more"`), `tooltip`, `menu`, `onMenu: event "act" {}`.

**Draggable**
- Reads: `payload: v<String> = ''`; `targets`: a list of `v<String>` over `length(['targets'])`, where empty means all targets; `width: v<double>` (feedback width; falls back to the context size, then 280); `disabled: v<bool>`; `child: optionalChild`; `feedback: optionalChild`.
- If `disabled` is true or `payload` is empty, it renders just the child.
- Feedback: rotated 0.035 rad, radius 10, shadow `0x55000000` blur 24 offset (0,10), carrying the DefaultTextStyle.
- While dragging, the child shows at opacity 0.3. A global "dragging targets" notifier lights up the matching drop targets.
- USED (wiboor): `payload`, `targets`, `width` (a switch of doubles), `child`, `feedback`, `disabled`.

**DropTarget**
- Reads: `id: v<String> = ''`, `hint: v<String>`, `color`, `hoverColor`, `radius: v<double> = 0`, `child: optionalChild`, `onDrop: handler`.
- Accepts a payload whose `targets` are empty or contain `id`.
- On drop it calls `onDrop({payload, to: id})`. Drop targets nest; the innermost one that accepts wins.
- Visuals:
  - Foreground border: accent 2 px while hovered over; accent at 0.35 alpha, 1.5 px, while a compatible drag is in the air.
  - Background: `hoverColor` (or accent at 0.10) while hovered over, otherwise `color`.
  - While hovered over, a `hint` pill is drawn at bottom 14 px: accent background, text 12 px w600.
  - The child is inside a `Stack`.
- USED (wiboor): `id`, `hint`, `radius: 12.0`, `onDrop: event "drop" {}`, `child`.

**Picture** (Rust port only; capability `picture: 1`)
- Reads: `path: v<String>`, `width: v<double>`, `height: v<double>` (ints accepted), `fit: BoxFit` string (`fill`, `contain`, `cover`, `fitWidth`, `fitHeight`, `none`, `scaleDown`; default `contain`).
- Named `Picture`, not `Image`, because `core.widgets` already has `Image` and both libraries are usually imported together.
- `path` is relative. It is looked up first in the plugin dir, then in the plugin data dir (`<state>/plugin-data/<id>`, the `dataDir` sent in `initialize`). Rejected: `..` anywhere, absolute paths, empty, files that do not exist.
- Decoded with the `image` crate (PNG and the other formats it reads). Decoded images are cached by `(path, mtime, len)`, at most 64 (the oldest is dropped), so a file rewritten in place shows the new image on the next render.
- Size: `width`/`height` when given, otherwise the image's natural size (one missing side follows the aspect ratio). The image is drawn centered in that box per `fit` and clipped to it. A missing or unreadable file renders an empty box of `width`×`height`.

**ZoomView additions** (Rust port only; capability `zoomFocus: 1`; the base widget is in `06-delta-2.4-to-2.7.md`)
- `command: "focus"` with `focus: {x, y, w, h}` (numbers, fractions 0..1 of the child box) and `commandRev`. Runs only when `commandRev` changes after mount, animated like `in`/`out`/`fit`. Without a valid `focus` map it does nothing.
- Target: scale = clamp(min(1/w, 1/h) × 0.9, 1.0, `maxScale`); the rect centre is moved to the view centre, then the translation is clamped so nothing outside the child shows. Focusing `(0,0,1,1)` gives the identity transform.
- `onMove`: fired after a user pan (drag, trackpad scroll) or wheel zoom settles (same 200 ms as `onZoom`), with extra `{scale, x, y}` (the transform: `x`/`y` are the layer translation in logical px). Programmatic commands never fire it. `onZoom` is unchanged.

##### C.4 Decoders reachable from the used widgets and arguments

**edgeInsets(key)** — used for `padding` and `margin`.
- Reads `[key,0..3]` as `v<double>`. If element 0 is null, the result is null.
- **A list of 1 to 4 doubles in the order [start, top, end, bottom]**, i.e. `EdgeInsetsDirectional.fromSTEB(a, b ?? a, c ?? a, d ?? b ?? a)`:
  - `[8.0]` gives 8 on all sides;
  - `[h, v]` gives start=h, top=v, end=h, bottom=v;
  - with 3 elements, bottom = top.
- **A bare number is NOT accepted:** indexing into a scalar gives `missing`, so the result is null.
- **Int elements are rejected**, giving null. Every real file uses 4-element double lists.

**color(key)**: `v<int>` → `Color(0xAARRGGBB)`. A double or a string gives null.

**alignment(key)**:
- Must be a map. Reads `x`, `start` and `y` (`v<double>`).
- Both `x` and `start` null, or `y` null, gives null.
- `start` present gives `AlignmentDirectional(start, y)`; otherwise `Alignment(x, y)`. Values range from -1 to 1.
- USED: `{x:0.0,y:-1.0}`, `{x:-1.0,y:0.0}`, `{x:1.0,y:0.0}`.

**decoration(key)**: switches on `v<String>([key,'type'])`; null gives null.

- `"box"` gives `BoxDecoration`:
  - `color: color`
  - `image: decorationImage` (unused)
  - `border: border`
  - `borderRadius: borderRadius`
  - `boxShadow`: a list of boxShadow (unused)
  - `gradient` (unused)
  - `backgroundBlendMode: enum BlendMode` (unused)
  - `shape: enum BoxShape = rectangle` (`rectangle`, `circle`)
- `"flutterLogo"` (`color`, `style`, `margin`) and `"shape"` (`color`, `image`, `gradient`, `shadows`, `shape: shapeBorder`) are not used.
- USED: only `type:"box"` with keys `color`, `border`, `borderRadius`, `shape:"circle"`.

**border(key)** = a list of 1 to 4 borderSide entries **[start, top, end, bottom]**.
- Gives `BorderDirectional(start: a, top: b ?? a, end: c ?? a, bottom: d ?? b ?? a)`. If element 0 is not a map, the result is null.
- USED: `[{color, width}]` for all four sides, and devtools' 4-element forms such as `[{style:"none"},{style:"none"},{style:"none"},{color: X, width: 1.0}]` (bottom only) and `[{style:"none"},{color,width},{style:"none"},{color,width}]` (top and bottom).

**borderSide(key)**: must be a map.
- `color: color = 0xFF000000`
- `width: v<double> = 1.0`
- `style: enum BorderStyle = solid` (`none`, `solid`). `{style:"none"}` means no side.
- **A side with `width: 2` as an int would fall back to 1.0.** Real files use doubles.

**borderRadius(key)** = a list of 1 to 4 **radius maps**, in the order [topStart, topEnd, bottomStart, bottomEnd].
- Gives `BorderRadiusDirectional.only(topStart: a, topEnd: b ?? a, bottomStart: c ?? a, bottomEnd: d ?? b ?? a)`.
- Each **radius(key)** is a map `{x: double, y?: double}`; a missing `y` equals `x` (elliptical(x, y)). Null `x` gives null.
- A bare number is NOT accepted.
- USED: `[{ x: 10.0 }]` everywhere, `[{ x: args.radius }]`, and devtools Bar's `[{x:1.0},{x:1.0},{x:0.0},{x:0.0}]` (rounded top only).

**textStyle(key)**: must be a map.
- `color`, `backgroundColor`: color.
- `fontSize`: double.
- `fontWeight`: fontWeight.
- `fontStyle`: enum, `normal` or `italic`.
- `letterSpacing`, `wordSpacing`: double.
- **`textBaseline` and `overflow` are read from the widget root (`['textBaseline']`, `['overflow']`) instead of the style — a bug.** The effect is that the style's `overflow` equals Text's `overflow`.
- `height`: double.
- `leadingDistribution`: enum.
- `locale`.
- `foreground`, `background`: paint.
- `shadows`: list of boxShadow.
- `fontFeatures`.
- `decoration`: textDecoration. This always returns non-null, `TextDecoration.none` when absent. It accepts a string `lineThrough|overline|underline` or a list of them.
- `decorationColor`, `decorationStyle`, `decorationThickness`.
- `fontFamily`: String.
- `fontFamilyFallback`: list of String.
- USED keys: `color`, `fontSize`, `fontWeight`, `fontFamily` (`data.theme.mono`), `height`, `letterSpacing`. All numbers in the real files are doubles.
- The style merges over the host's `DefaultTextStyle` (fg, 13).

**fontWeight(key)**:
- `v<int>` in {100, 200, ..., 900} gives w100 to w900.
- Otherwise a string: `w100`..`w900`, `normal` (w400) or `bold` (w700).
- Anything else gives null.
- USED: `"w400"`, `"w500"`, `"w600"`, `"w700"`.

**enumValue(values, key)**: `v<String>` compared exactly, case-sensitive, to the enum's short name (`Enum.toString().split('.').last`). No match gives null, then the widget default.

**gridDelegate(key)**: switches on `type`.
- `"fixedCrossAxisCount"`: `crossAxisCount: int = 2`, `mainAxisSpacing = 0`, `crossAxisSpacing = 0`, `childAspectRatio = 1.0`, `mainAxisExtent: double?`.
- `"maxCrossAxisExtent"`: `maxCrossAxisExtent = 100.0`, the same spacings, `childAspectRatio = 1.0`, `mainAxisExtent?`.
- USED: `maxCrossAxisExtent`.

**duration(key)**: `v<int>` milliseconds; null gives the default 200 ms.

**curve(key)**: a string name. Accepted: `linear`, `decelerate`, `fastLinearToSlowEaseIn`, `ease`, `easeIn`, `easeInToLinear`, `easeInSine`, `easeInQuad`, `easeInCubic`, `easeInQuart`, `easeInQuint`, `easeInExpo`, `easeInCirc`, `easeInBack`, `easeOut`, `linearToEaseOut`, `easeOutSine`, `easeOutQuad`, `easeOutCubic`, `easeOutQuart`, `easeOutQuint`, `easeOutExpo`, `easeOutCirc`, `easeOutBack`, `easeInOut`, `easeInOutSine`, `easeInOutQuad`, `easeInOutCubic`, `easeInOutCubicEmphasized`, `easeInOutQuart`, `easeInOutQuint`, `easeInOutExpo`, `easeInOutCirc`, `easeInOutBack`, `fastOutSlowIn`, `slowMiddle`, `bounceIn`, `bounceOut`, `bounceInOut`, `elasticIn`, `elasticOut`, `elasticInOut`. Default `fastOutSlowIn`. Not used explicitly.

**Reachable but unused**, so a minimal Rust implementation can skip them:
- boxConstraints: `{minWidth, maxWidth, minHeight, maxHeight}` with defaults 0, ∞, 0, ∞.
- matrix (16 doubles; element 15 is required).
- boxShadow: `{color, offset: {x,y}, blurRadius, spreadRadius}`.
- gradient: linear, radial or sweep.
- decorationImage, imageProvider.
- strutStyle, textHeightBehavior, locale, paint, maskFilter, shader, textDecoration, fontFeature.

---

#### D. Per-file inventory

All files import `core.widgets` and `maestria`; the two pomodoro files also import `core.material`. No file uses `state` or `set state`. Every event is `"act"`, except wiboor's `"drop"`.

##### docker/ui/sidebar.rfwtxt (root `root`)

**Declares:** root, Head, Label, Avatar, TitleText, SubText, Actions, Pill (root switch), ContainerRow, ProjectRow, TermRow.

**Local widgets:** Column, Row, Expanded, ListView, Padding, SizedBox, Text, Stack, Opacity, Positioned (`end`, `bottom`), Container, Center, Rotation (`turns`, `duration: 140`); maestria Svg, Glyph, IconButton, MenuButton, Pressable.

**data keys:**
- `head.title`, `head.sub`, `head.menu`
- `rows[]` with `.kind` (`"label"`, `"project"`, `"term"`, or a container by default), `.title`, `.sub`, `.svg`, `.dot` (`"none"`, `"green"`, `"red"`, `"yellow"`), `.dim`, `.selected`, `.strong`, `.indent` (an int; the switch matches `1`), `.open`, `.toggle`, `.action`, `.pill`, `.actions[]` (`{icon, tooltip, action}`), `.menu`
- theme: accent, active, dim, faint, fg, green, hover, onAccent, red, sidebar, yellow

**Constructs:**
- `...for row in data.rows: switch row.kind {...}` (a loop with a switch template)
- `...for act in args.row.actions`
- int and bool switches; nested switches (`switch args.selected { true: X, default: switch args.dim {...} }`)

**Events:** `"act" {a: "atualizar"|"janela"|args.row.action|args.row.toggle|act.action}` and `"act" {}` (menus, which deliver `{action}`).

**Maestria args:**
- MenuButton: `icon`, `tooltip`, `menu`, `onMenu`
- IconButton: `icon`, `tooltip`, `onPressed`, `color`
- Pressable: `radius`, `color`, `hoverColor`, `onTap`, `menu`, `onMenu`, `padding`, `child`
- Svg: `svg`, `width`, `height`
- Glyph: `icon`, `size`, `color`

##### ssh/ui/hosts.rfwtxt (root `root`)

**Declares:** root, Head, Toolbar, Action, Crumb (root switch on `data.crumb.group`), Section, Tile, Avatar.

**Local widgets:** Column, Row, Expanded, ListView, Padding, Text, Container (with `clipBehavior: "antiAlias"`), SizedBox, Center, Wrap, GridView (maxCrossAxisExtent), Stack, **Positioned (`right`, ignored; `bottom`)**; maestria Glyph, IconButton, MenuButton, Pressable.

**data keys:**
- `head.title`, `head.sub`, `head.newHost`
- `crumb.group` (a string, `""` meaning none), `crumb.menu`
- `sections[]` with `.title`, `.count` (string), `.empty` (string, `""` meaning show the grid), `.items[]`
- each item `t` has `title`, `sub`, `icon`, `tone` (accent, blue, cyan, green, yellow, red, purple, magenta, faint, or default), `status` (`""`, `"green"`, `"red"`), `action`, `menu`, `edit`
- `hint`
- theme: accent, active, blue, border, cyan, dim, faint, fg, green, hover, magenta, onAccent, purple, red, sidebar, yellow

**Events:** `"act" {a: "buscar"|"atualizar"|args.a|"raiz"|args.t.action|args.t.edit}` and `"act" {}` (menus).

**Maestria args:**
- IconButton: `icon`, `tooltip`, `onPressed`, `size`, `color`
- MenuButton: `icon`, `tooltip`, `menu`, `onMenu`
- Pressable: `radius`, `color`, `hoverColor`, `padding`, `onTap`, `menu`, `onMenu`, `child`
- Glyph: `icon`, `size`, `color`

##### pomodoro/ui/float.rfwtxt (no root sent, so the default `root`; same file in pomodoro_dart)

**Declares:** root, Ring.

**Local widgets:** Padding, Row, SizedBox, Expanded, Column, Text, Stack (`fit: "expand"`), Center; material CircularProgressIndicator; maestria Glyph, IconButton, MenuButton.

**data keys:** `clock`, `label`, `running` (bool), `progress` (double, never integral), `color` (an ARGB int), `phase` (`"focus"` or other), `menu`; theme: active, dim, faint, fg, mono.

**Events:** `"act" {a: "alternar"|"pular"}` and `"act" {}` (menu).

**Maestria args:**
- IconButton: `icon` (switch), `tooltip` (switch), `color`, `onPressed`
- MenuButton: `icon`, `tooltip`, `menu`, `onMenu`
- Glyph: `icon` (switch), `size`, `color`

##### pomodoro/ui/sidebar.rfwtxt (default root `root`)

**Declares:** root, Segmented, Dial, Controls, Today, Ruler, Preset, Stepper, Toggle, Caption, IconChip, Swatch.

**Local widgets:** ListView, SizedBox, Center, Row, Expanded, Padding, Wrap, Container (with `color`), Column (`mainAxisSize: "min"`), Stack (`fit: "expand"`), Text, Align (alignment switch); material CircularProgressIndicator; maestria Glyph, IconButton, Pressable, Tooltip.

**data keys:**
- `phases[]` (`{active, id, label}`)
- `progress`, `color`, `clock`, `label`, `running`
- `dots[]` (bare bools)
- `focusColor`, `today`
- `presets[]` (`{active, id, label}`)
- `steppers[]` (`{label, atMin, atMax, id, value}`; `value` is a string)
- `toggles[]` and `panel[]` (`{id, label, on, hint[] (list of strings)}`)
- `anim.phases`, `anim.icons[]` (`{label, active, id, emoji}`), `anim.colors[]` (`{label, id, value (ARGB int), active}`), `anim.toggles`, `anim.color`, `anim.test`
- theme: accent, active, border, canvas, dim, faint, fg, hover, mono, onAccent

**Constructs:** 10 loops, including `...for h in args.t.hint: Text(text: h)`, `switch d { true: ... }` on a bare loop variable, and loops directly inside the ListView children list mixed with fixed items.

**Events:** `"act" {a: ...}` and `{a: ..., id: ...}` (fase/cena, preset, menos/mais, toggle, icone, cor, testar, zerar, alternar, pular).

**Maestria args:**
- IconButton: `icon`, `tooltip`, `size`, `extent`, `onPressed`, `disabled`
- Pressable: `radius`, `color`, `hoverColor`, `onTap`, `child`
- Tooltip: `message`, `child`
- Glyph: `icon`, `size`, `color`

##### argocd/ui/argo.rfwtxt (roots `home` and `tree`)

**Declares:** home, tree, Head, Stats, Notice (root switch), Dot (root switch), Pill, Node, Tile, Crumb (root switch), ClusterTile.

**Local widgets:** Column, Row, Expanded, SingleChildScrollView (vertical and horizontal, `primary: false`, padding), Wrap, SizedBox, Stack, Positioned (`start: n.x`, `top: n.y`; and `end`/`bottom`), Padding, Container, Center, Text, Spacer; maestria Svg, Glyph, IconButton, Pressable.

**data keys:**
- `notice.text`, `notice.tone`
- `crumb.cluster`
- `clusters[]` (`{a, status, name, sub, subTone, buttons[] {icon, tip, a}}`)
- `apps[]` (`{a, health, icon, name, healthText, sync, syncText, op, opTone, rows[] {k, v}, sync_a, refresh_a, browser_a}`)
- `graph.w`, `graph.h` (must be doubles), `graph.svg`, `graph.nodes[]` (`{x, y, sel, a, health, abbr, title, sub, sync}`)
- `head.title`, `head.sub`, `head.buttons[]` (`{icon, tip, tone, a}`)
- `stats[]` (`{a, tone, text}`)
- theme: accent, active, bg, border, dim, faint, fg, green, hover, purple, red, yellow

**Constructs:** two consecutive loops in one list (`...for c in data.clusters: ...`, `...for app in data.apps: ...`), and a loop inside a Row's children after fixed items.

**Events:** only `"act" {a: ...}`. No menus.

**Maestria args:**
- Svg: `svg`, `width`, `height`
- Glyph: `icon` (including `"icons/argo.svg"`), `size`, `color`
- IconButton: `icon`, `tooltip`, `color`, `onPressed`
- Pressable: `radius`, `color`, `hoverColor`, `padding`, `onTap`, `child`

##### wiboor/ui/kanban.rfwtxt (root `root`)

**Declares:** root, Head, Toolbar, PillBone, Scopes, Scope, Pill, Notice (root switch), Dot, Wide, Lane, LaneShape (root switch), LaneBody (root switch), LaneBox, LaneGhost, LaneStripe, LaneHead, Folded, Skeletons, Skeleton, Block, Bone, Empty, DragCard, Card, Kind, Prio (root switch), Bar (root switch), Who (root switch), Due (root switch), Meta (root switch), Claude (root switch).

**Local widgets:** Column, Row (`crossAxisAlignment: "stretch"`), Expanded (child is a switch), Align, SingleChildScrollView (horizontal), Wrap (`crossAxisAlignment: "center"`), Container (`clipBehavior`, `color`, `margin`), SizedBox (child `args.child` or a switch), Padding, Text (`textAlign: "center"`), Center, Spacer, Stack, Positioned (`start: -84.0`, `top`, `width`, `height`), Rotation (`turns: 0.25`), ListView, IntrinsicHeight, FractionallySizedBox; maestria Glyph, IconButton (with `disabled`, `extent`), MenuButton, Pressable, Tooltip, Draggable, DropTarget.

**data keys:**
- `head.title`, `head.sub`, `head.menu`, `head.busy` (bool), `head.scopes[]` (`{action, active, icon, label}`)
- `stats[]` (`{action, active, tone, text}`)
- `statsSkeleton` (bool)
- `notice.text`, `notice.tone`, `notice.retry` (bool)
- `cols[]`:
  - `{slot, slots[], id, hint, collapsed, skeleton, count (string; "0" means empty), cards[], peek[], more, tone, icon, label, sortLabel, menu, toggle, empty, invite}`
  - each card: `{id, targets[], fixed, running, action, menu, prioTone, kind, num, prio, title, sub, bar {show, value (double), label, done}, claude {text, tone, action}, who {initials, name, tone}, due, dueTone, checks, comments, files, acts[] {icon, tooltip, primary, action}}`
- `drag` (bool)
- `layout.width` (`"narrow"`, `"wide"`, or other)
- theme: accent, active, bg, blue, border, canvas, cyan, dim, faint, fg, green, hover, magenta, mono, onAccent, purple, red, sidebar, yellow

**Constructs:**
- `...for w in switch data.statsSkeleton { true: [78.0, 112.0, 96.0], default: [] }:` (a loop over a switch)
- `switch args.line { 0.0: ..., default: ... }` (a double case key)
- `borderRadius: [{ x: args.radius }]`
- `child: args.child` (a widget passed as an argument)
- nested DropTargets

**Events:**
- `"act" {a: "buscar"|"atualizar"|"personalizar"|"nova"|args.s.action|args.pill.action|args.col.toggle|args.card.action|act.action|args.action}`
- `"act" {}` (menus, which deliver `{action}`)
- `"drop" {}`, which delivers `{payload, to}`

**Maestria args:**
- Draggable: `payload`, `targets`, `width`, `child`, `feedback`, `disabled`
- DropTarget: `id`, `hint`, `radius`, `onDrop`, `child`
- MenuButton: `icon`, `tooltip`, `menu`, `onMenu`
- IconButton: `icon`, `tooltip`, `onPressed`, `disabled`, `size`, `extent`, `color`
- Pressable: `radius`, `color`, `hoverColor`, `padding`, `onTap`, `menu`, `onMenu`, `child`
- Tooltip: `message`, `child`
- Glyph: `icon` (names and `icons/*.svg`), `size`, `color`

##### flutter/ui/devtools.rfwtxt (root `root`)

**Declares:** root, Head, Tab, Offline, Card, CardTitle, Note, Warn (root switch), Stats, Button, Segmented, Key, Toggle, Perf, FrameChart, ScaleLabel, FrameBar, Bar, Cpu, CpuRow, Pct, Mem, MemBar, Net, NetRow, NetDetail.

**Local widgets:**
- Column, Row, Expanded (`flex` as an int literal or from data), Container (`alignment`, `color`, border with `style: "none"` sides), Wrap, SizedBox, Padding, Text (`textAlign: "center"|"end"`), Center, Stack, Positioned (`start`, `end`, `top`, `bottom`)
- ListView (`scrollDirection: "horizontal"`, `reverse: true`, `itemExtent: 9.0`)
- GestureDetector (`onTap`, `behavior: "opaque"`), Flexible
- maestria Glyph, IconButton, Pressable, Tooltip

**data keys:**
- `head`: `dot`, `status`, `tabs[]` (`{action, active, icon, label}`), `live`, `tab` (`"cpu"`, `"mem"`, `"net"`, or other), `offline`
- `perf`: `warn`, `stats[]` (`{hint, label, value, tone}`), `recording`, `detail`, `hint`, `togglesNote`, `toggles[]` (`{hint, action, on, label}`), `worst[]` (`{action, sel, total, name, split}`), `worstEmpty`, `bars[]` (`{action, tip, sel, bt, b, bc, rt, r, rc}`; `bt`, `b`, `rt`, `r` are ints used as flex), `scaleTop`, `scaleMid`
- `cpu`: `warn`, `buttonIcon`, `button`, `primary`, `status`, `filters`, `sorts` (each a list of `{action, active, label}`), `empty`, `rows[]` (`{action, self, selfFill, selfRest, bySelf, total, totalFill, totalRest, byTotal, name, where}`; the `*Fill` and `*Rest` values are ints)
- `mem`: `stats`, `scale`, `samples[]` (`{tip, empty, gc, free, ext, used}`; ints), `from`, `filters`, `markIcon`, `mark`, `marked`, `note`, `deltaHead`, `classes[]` (`{action, name, lib, count, size, delta, tone, deltaCount}`)
- `net`: `status`, `paused`, `empty`, `rows[]` (`{action, sel, method, status, tone, path, host, time, size}`), `open`, `detail.title`, `detail.kv[]` (`{k, v, tone}`), `detail.sections[]` (`{title, action, text}`)
- theme: accent, active, bg, blue, border, canvas, cyan, dim, faint, fg, green, hover, mono, onAccent, red, sidebar, yellow

**Events:** only `"act" {a: ...}`.

**Maestria args:**
- IconButton: `icon`, `tooltip`, `onPressed`, `size`, `extent`
- Pressable: `onTap`, `hoverColor`, `radius`, `color`, `padding`, `child`
- Tooltip: `message`, `child`
- Glyph: `icon` (including `"icons/close.svg"`), `size`, `color`

##### Union of local widgets actually needed

- **core.widgets:** Align, Center, Column, Container, Expanded, Flexible, FractionallySizedBox, GestureDetector, GridView, IntrinsicHeight, ListView, Opacity, Padding, Positioned, Rotation, Row, SingleChildScrollView, SizedBox, Spacer, Stack, Text, Wrap.
- **core.material:** CircularProgressIndicator.
- **maestria:** all 8 (Svg, Glyph, IconButton, Tooltip, Pressable, MenuButton, Draggable, DropTarget).

---

#### E. rfw features no real file uses (safe to skip, or implement minimally)

**Language:**
- stateful widgets (`widget X { ... } =`), `state.*` references, `set state.x = v`, state depth semantics
- widget builders `(x) => ...` and builder-argument references
- `null` map values
- single-quoted strings, escape sequences, `\uXXXX`
- block comments `/* */`
- exponent doubles
- string or integer parts in references (`data.rows.0`, `data."key"`), and string keys in maps or arguments (all keys are bare identifiers)
- lists of handlers in an `on*` argument
- references to an outer loop variable from inside an inner loop within the same expression
- `import` with string parts, or library names other than `core.widgets`, `core.material` and `maestria`
- switch with no `default` (every real switch has one)
- switch case keys that are references, lists or maps

**Widgets:**
- core.widgets: AnimationDefaults, AspectRatio, ClipRRect, ColoredBox, DefaultTextStyle, Directionality, FittedBox, Icon, IconTheme, IntrinsicWidth, Image, ListBody, Placeholder, SafeArea, Scale, SizedBoxExpand, SizedBoxShrink
- core.material: everything except CircularProgressIndicator (AboutListTile, AppBar, ButtonBar, OverflowBar, Card, Divider, Drawer, DrawerHeader, DropdownButton, ElevatedButton, FloatingActionButton, InkResponse, InkWell, LinearProgressIndicator, ListTile, Material, OutlinedButton, Scaffold, Slider, TextButton, VerticalDivider)

**Arguments and decoders never passed:**
- decoration types `shape` and `flutterLogo`, and `boxShadow`, `gradient`, `image`, `backgroundBlendMode` inside box decorations
- `foregroundDecoration`, `constraints`, `transform`, `transformAlignment` on Container
- `curve` anywhere (only `duration: 140`, on Rotation)
- `onEnd` handlers
- `strutStyle`, `textHeightBehavior`, `locale`, `softWrap`, `textScaleFactor`
- textStyle `fontStyle`, `decoration`, `shadows`, `foreground`, `background`, `fontFeatures`, `fontFamilyFallback`, `wordSpacing`, `backgroundColor`
- GestureDetector handlers other than `onTap`
- Pressable `onDoubleTap`
- the `fixedCrossAxisCount` grid delegate
- Stack `alignment` and `clipBehavior`
- Wrap `direction`, `alignment`, `runAlignment`
- Column/Row `textDirection`, `verticalDirection`, `textBaseline`
- Flexible `fit`, Spacer `flex`, Center factors
- the integer form of `fontWeight` (only `"w400"` to `"w700"` strings are used)
- `alignment` with `start`

---

#### Gotchas for the Rust implementation

1. **Tokenizer strictness:**
   - Only space and LF count as whitespace; TAB and CR are errors.
   - `//` must not be glued to a preceding identifier, number or string.
   - A raw LF inside a string is an error.
   - `1.` and `.5` are invalid.
2. **Int vs double are distinct at every level** (literals, JSON data, decoders), but **switch matching compares numbers by value**, so int 1 matches case 1.0.
3. **edgeInsets, border and borderRadius only accept lists**, in the order [start, top, end, bottom] (for borderRadius: [topStart, topEnd, bottomStart, bottomEnd]), with the fallback rule `b ?? a`, `c ?? a`, `d ?? b ?? a`. Radius entries are `{x, y?}` maps.
4. **Positioned uses `start`/`end`, not `left`/`right`.** The `right: 0.0` in ssh Avatar is ignored by real rfw.
5. **Opacity defaults to 0.0.** CircularProgressIndicator with a null `value` spins.
6. **Missing values are a distinct sentinel:**
   - A loop over something that is not a list yields nothing.
   - A missing required `child` renders an ErrorWidget; `childList` on missing yields nothing.
   - `Text` with no string renders `""`.
7. **Menu entries with no `action` string render as dividers.** The menu choice is delivered as an extra `action` argument appended to the event's own arguments.

### 3.5 Host-specific RFW notes (decisions for the Rust port)

- Event args containing `missing` values: drop those keys when converting to JSON (Dart would pass a
  non-JSON sentinel). Real files never hit this when data is complete.
- `Positioned(right:, left:)` are ignored by rfw 1.1.4 (only `start/top/end/bottom`). The ssh avatar
  status dot relies on this accidentally; reproduce rfw behavior (ignore `left`/`right`).
- Theme object pushed as `data.theme` (overrides any plugin-sent `theme` key, which is skipped):
  `{dark: bool, canvas, bg, sidebar (=bgSidebar), hover (=bgHover), active (=bgActive), border, fg,
  dim (=fgDim), faint (=fgFaint), accent, onAccent (0xFFFFFFFF if accent is "dark" by luminance else canvas),
  green, yellow, red, purple, blue, cyan, magenta (ANSI), mono: "<mono font family string>"}`; colors
  are ints `0xAARRGGBB` (need 64-bit ints / u32). Re-pushed on theme change.
- Float content: RFW if a library exists, else blocks in bare mode.


---

## 4. Views, sidebar tab, screen, floats, modals, dialogs, commands, keybindings, themes

### 4.1 Floats (`float.show` / `float.update` / `float.hide`)

- Key `"<pluginId>/<id>"`; backing tab id `float:<pluginId>/<id>`, kind `plugin`, view title = plugin name.
- New float defaults: width 200, height 64, start corner `bottomRight`.
- `width` clamped to [80, 480], `height` to [32, 320] (numbers, int or double).
- `corner` (show only): `topLeft|topRight|bottomLeft|bottomRight`; only sets the *start* corner,
  used until the user drags it. `float.update` has no `corner`.
- Content application order in `_apply`: size; if rfw → `setRfw(library, root, data)`; if `blocks`
  present → `rfwLibrary = null` then `setBlocks`. So sending `blocks` switches back to blocks.
- `float.show` always returns `{shown:true}` (even for an already shown float: updates it).
  `float.update` returns `{shown:false}` if not shown (no-op). `float.hide` removes it.
- Events from float content → `view.action` with `viewId = <float id>`.
- Rendering: layer above sidebar + panes (below status bar), transparent outside floats. Renders RFW
  if `rfwLibrary != null` else blocks in bare (sidebar-size) mode. Rounded card `Mx.radius`,
  border (accent while dragging), drop shadow.
- Placement: `spot = {corner, dx, dy}` (distance from that corner's two edges). Default spot =
  `{start, margin, margin}` with margin = `Mx.gap`. Position clamped inside window with margin.
  Dragging = pan gesture (a tap still reaches inner buttons). On drop: corner = quadrant of the
  float's center; dx/dy distances to that corner's edges; values `< margin + 28` snap to `margin`.
  Animated 160 ms easeOutCubic when not dragging.
- Spots persisted in config (rounded): `{ "<plugin>/<id>": { "corner": "bottomRight", "dx": 12.0, "dy": 12.0 } }`;
  invalid entries skipped on read. Spots survive hide.
- Floats are dropped when the plugin is disabled, uninstalled, or reinstalled; hidden (not dropped)
  while the plugin is crashed. Not restored on app restart.

### 4.2 Views (windows) — `view.open` / `view.update` / `view.close`

Model `PluginView { pluginId, pluginName, id(viewId), title, blocks, rfwLibrary?, rfwRoot="root",
rfwData{}, revision, rfwRevision, consoles{id→PluginConsole} }`.

`setRfw(library?, root?, data?)`: library replaces when non-null; root replaces when non-null; data
is **shallow-merged** (each top-level key replaces), `rfwRevision++`.

`view.open(viewId, title=plugin name, blocks, rfw, modal)`:
1. If the current modal is this view: set title, `setBlocks(blocks)` (blocks = `[]` if absent),
   apply rfw; stay modal; return its tabId.
2. Else if a tab with (pluginId, viewId) exists (grid or modal): set title, `setBlocks`, apply rfw;
   switch to its screen; if not on screen place it beside the previously focused tab; focus it.
3. Else create tab (id from the app's tab id sequence, folder = loose, cwd = loose root, kind `plugin`)
   with blocks, apply rfw. `modal:true` → it becomes `pluginModal` (replacing any other modal; not
   added to tabs). Else append to tabs and place beside the focused tab.
**NOTE**: `view.open` does not clear `rfwLibrary` when given blocks — a view that once had an RFW
library keeps rendering RFW after a `view.open` with only blocks (only `view.update`/`float`/
`sidebar.update` with `blocks` clear it). Reproduce for compatibility.

`view.update(viewId, title?, blocks?, rfw?)`: not found → `{open:false}`. title if non-null; rfw
applied; if `blocks` key present → `rfwLibrary=null`, `setBlocks`. `{open:true}`.

`setBlocks`: replaces list, `revision++`, re-adopts consoles: walk blocks recursively into
`row`/`card`/`columns` children; for each `console` with string id: get-or-create console; `max`
(num) sets cap; if the block has the `lines` key → replace lines. Consoles whose id disappeared are
disposed. (A console block without `lines` keeps its lines.)

Console lines: string → `{text}`; `{spans:[...] , tone?}` → spans (each `{text, tone?, bold?}` or
string), text = concatenation; `{text, tone?}` → `{text: "${text ?? ''}", tone}`; others ignored.
`append` accepts a list or a single line; trims oldest beyond `max` (default 5000).
`view.appendLines`/`view.clearLines` target consoles directly (no view revision bump).

`view.close`: closes the modal or the tab. Closing a view tab also closes embedded terminals that its
blocks reference (`terminal` blocks with tabId owned by the plugin and embedded, recursively).

Disabling/uninstalling/reinstalling a plugin closes its modal, its view tabs and floats.

Where a view lives: plugins with `contributes.screen` open views in the plugin's own screen (pane
tree); others in the sessions grid. See §4.3.

Pane header for a view: plugin icon (17px), title, pane key hint, subtitle `Plugin · <pluginName>`
(when width ≥ 190), settings button `Configurar <name>` (if plugin has settings), screen
release/return button (if plugin has screen: tooltips `levar pra tela das sessões, ao lado do painel em foco lá` /
`devolver pra tela do <name>`), pin button, close `Fechar esta janela`.
Empty blocks (no rfw) → centered `Nada pra mostrar ainda`.

### 4.3 Modal views (`modal:true`)

- One modal at a time (`pluginModal`), outside `tabs`. Found by `_pluginTab` (so `view.update`,
  `view.close`, `view.appendLines` work on it).
- UI: dim backdrop (black 42 %, fade 140 ms), centered card width `clamp(W-48, 320, 820)`,
  height `clamp(H-48, 240, H*0.88)`; header: plugin icon, title, subtitle, button
  `abrir como painel`, close button tooltip `fechar (esc)`. Body = blocks/RFW without pane frame.
- Close on x, Esc, backdrop click → `pluginModal = null`; plugin learns via next `view.update`
  returning `{open:false}`.
- `abrir como painel` (dock): appended to tabs, placed beside focused tab, focused. From then on a
  regular view; a later `view.open` (modal or not) just brings it forward.
- Opening a modal for a view already in the grid does not move it (rule 2 above wins).

### 4.4 Dialogs requested by plugins

**Quick pick** (`window.pick`): dialog (barrier `0x33000000`) at alignment (0,-0.6), width 520:
title line (dim, 12px), filter field (hint = placeholder or `Filtrar`), list (max height 360).
Filter: lowercase query split on whitespace; every term must be contained in `"<label> <detail>"`
lowercased. Arrow up/down cycle the highlight; Enter picks highlighted; hover highlights; click
picks. Empty → `Nada com esse nome`. Esc/outside → null.

**Quick input** (`window.input`): same card; field prefilled with `value` fully selected; help line
= `prompt` or `enter pra confirmar, esc pra cancelar`. Enter → trimmed text, empty → null.

**Form** (`window.form`): parse rules (errors → -32602):
- `fields` list; item not object → `cada campo é um objeto`; id missing → `campo sem "id"`;
  duplicate → `campo repetido: "<id>"`; type other than `text`/`select` →
  `o campo "<id>" tem "type": "<t>" — vale "text" ou "select"`; none → `"fields" precisa de ao menos um campo`.
- field: `{id, label?, placeholder?, value? (string, or number → "$n", else ""), type?, options?
  (string | {value,label?}; invalid ignored), half?, required?, showIf?: {field, value} (both strings)}`.
- buttons: items with non-empty string `action`; `{action, label (default action), style: "primary"|"danger"|""}`.
  None → default `[{action:"ok", label:"ok", style:"primary"}]`.
- `title` default plugin name; `error` trimmed non-blank → red text under fields.
UI: AlertDialog width 480; select initial = value if it is an option else first option (or `""`);
fields hidden unless `showIf` matches (`values[field] == value`); consecutive `half` fields share a row
(a lone half takes half the row). Buttons: danger ones on the left (red text buttons); right side:
`cancelar` then non-danger buttons (primary = filled). Pressing a non-danger button with a visible
`required` field empty → error under that field `falta <label or 'este campo'>`, dialog stays.
Enter in a text field presses the first `primary` button, else the last non-danger one.
Result `{action, values}` with **all** fields (including hidden) as strings. Cancel/Esc/outside → `null`.

**Settings dialog** (from plugin card or view header): title `Configurar <name>`; per setting row:
title + description, control by type (boolean switch; select dropdown; number text accepting only
numbers (invalid → red, not saved); string text; color: 6 presets
`#4FC1FF #89D185 #F48771 #CCA700 #C586C0 #9DA5B4`, sample `Aa log`, hex field hint `Do tema`,
valid = empty, `#RRGGBB`/`#AARRGGBB`, or one of `red green yellow accent purple faint dim`),
reset button `Voltar ao padrão` when value != default. Group headers. Then section `Atalhos` listing
commands with keys; a key taken by the app keymap is struck through with
`Já é "<app action label>" no app — este comando fica sem tecla`. Footer `Vale na hora`, `Pronto`.
Every change saves immediately and sends `settings.changed`.

**Log dialog**: `Log de <name>`, newest first, `Copiar` (joined by `\n`), `Fechar`; empty → `Nada ainda`.

### 4.5 Commands

Where they appear:
- Pane context menu → submenu `Plugins` with every active plugin's commands (`plugin:<fullId>`), run
  with `on = that tab`.
- Generic plugin sidebar tab (no `contributes.sidebar`): section `Janelas` (open views + owned
  terminals, `Nenhuma janela aberta`) and section `Comandos`. Command rows show `shortTitle`: if the
  title is `"<X>: <rest>"` with `X` equal (case-insensitive) to the plugin name, only `<rest>` is shown
  (`flutter: hot reload` → `hot reload`; `git: outro prefixo` kept in plugin Flutter).
- Footer buttons: commands with `sidebar:true` whose plugin has **no** rail icon, max 3, in plugin
  order; spinner while busy; icon via `PluginGlyph`.
- Keybindings (§4.6). Screen `home` (§4.8).

Execution (`runPluginCommand(command, on?)`), tab = on ?? focused; folder = tab.folder ?? focused
folder; values `{cwd, folder, sessionId, title, pluginDir}`:
- `terminal`: open a shell tab in folder/cwd running `expand(run, quote:true)`.
- `background`: run `expand(run, quote:true)` via shell in cwd; said = last non-blank line of stdout
  (ok) or stderr (fallback stdout); banner `<title>: <said or 'feito'>` or sticky
  `<title> falhou (código <n>)[: <said>]`.
- `session`: requires focused claude tab not exited/hibernated else banner
  `<title>: precisa de uma sessão do Claude rodando em foco`; submits `expand(send, quote:false)`.
- `plugin`: `invoke` with context = values + `tabId` if tab; errors → sticky banner `<title>: <msg>`.

`expand(template, values, quote)`: regex `\$\{(\w+)\}`; unknown names left as is; values
single-quoted shell-style when `quote` (`Sh.q`: `'` + value with `'` → `'\''` + `'`).

`command.busy` → `busy` set of fullIds; rail icon shows accent dot while any command of that plugin
is busy; cleared on stop/crash.

Wire note: host JSON is compact (`jsonEncode`, no spaces: `"method":"initialize"`) and request ids
are plain integers; test fixtures (and naive plugins) grep the raw line for `"method":"x"` and `"id":<n>`.

### 4.6 Keybindings

- Chord string: `+`-separated; last part = key, others ∈ {`ctrl`,`alt`,`shift`,`meta`} (any other
  → unparseable). Key names: single printable char (its code point; letters lowercase), or
  `tab enter space backspace delete escape left right up down home end pageup pagedown f1..f12 plus`.
- `forHost`: on Linux, `meta` → `ctrl+shift` (alt kept). Labels: Linux `Ctrl+Shift+Alt+Super+<Key>`
  order (ctrl, shift, alt, super); macOS glyphs `⌃⌥⇧⌘<KEY>`.
- Rejection (warning in log, command keeps no key): unknown key `essa tecla não dá pra usar num atalho`;
  reserved (Linux: Ctrl+Shift+C/V/E/U, Ctrl+Alt+T, Ctrl+Alt+Del, Ctrl+Alt+arrows, Alt+F4, Alt+Tab;
  macOS: ⌘Q ⌘W ⌘H ⌘M ⌃⌘F ⌥⌘H ⌘V ⌘C) with their reasons; session slot keys (`é a tecla da <n>ª sessão da lateral`,
  Alt+1..9 Linux / ⌘1..9 mac); no ctrl/meta/alt and not F-key →
  `precisa de Ctrl ou Alt — sem modificador a tecla sumiria do terminal` (mac: `precisa de ⌘, ⌃ ou ⌥ — …`);
  ctrl+letter alone → `o terminal manda <label> pro processo como código de controle`.
- Dispatch: on key-down in a terminal pane (and global handler), **after** the app keymap: if the app
  keymap matches the event, plugins never see it. First active command (plugin order, manifest order)
  whose chord hits. Hit test: exact logical match, **or** same physical key position for a–z / 0–9
  (US HID usage 0x04+…, digits 0x1e…, 0 = 0x27) with exactly equal modifier state (macOS dead keys).
  Log `tecla <label> → <commandId>`. Runs `runPluginCommand(command, on: that tab)`.
- Conflicts with app keymap are shown in the settings dialog (§4.4) and the plugin card's ⋮ turns yellow.

### 4.7 Themes contributed by plugins

Rebuilt on scan/enable/disable for active plugins only. Each path is relative to plugin dir; JSON
object (`não é um objeto`). Palette schema (all `#RRGGBB` or `#AARRGGBB`; 8-digit = ARGB):
`id`*, `label`*, `dark` (default true unless `false`), `canvas bg bgSidebar bgHover bgActive border
fg fgDim fgFaint accent green yellow red purple`*, `ansi`* object with `black red green yellow blue
magenta cyan white`* and optional `brightBlack … brightWhite` (fallback to normal). Errors:
`"<key>" precisa ser uma cor #RRGGBB` (prefix `ansi.` for ansi keys), `"id" é obrigatório`,
`"label" é obrigatório`, `"ansi" é obrigatório`; id equal to a built-in or retired theme →
`"<id>" é o id de um tema embutido`; duplicate among plugins → `outro plugin já tem um tema "<id>"`.
Errors are logged as `tema <path>: <message>`. Themes appear in Appearance under the "de plugins"
block. Built-in ids: maestria, nord, gruvbox-dark, dracula, rose-pine-dawn, ayu-dark,
solarized-dark, everforest-dark, monokai-pro, github-light, gruvbox-light, claude-dark, night-owl,
slate, graphite, synthwave-84, cobalt, high-contrast, zenburn. Retired alias map (keys are also
reserved): `tokyo-night`→`maestria`, `catppuccin-frappe`→`catppuccin-mocha`, `rose-pine`→`catppuccin-mocha`,
`chatgpt-dark`→`graphite`. `byId(id)` = find(id) in built-ins then plugin themes, else find(alias
target), else `maestria`. So once the catppuccin plugin provides `catppuccin-mocha`, configs saved
with `catppuccin-frappe`/`rose-pine` open in it.

### 4.8 Sidebar rail, plugin tab, own screen

Rail plugins (one icon each): active plugins with `sidebar` or `screen` or (`icon` and ≥1 command),
plus any plugin with an open view or owned terminal; ordered by persisted `railOrder` then plugin
order; drag-to-reorder. Icon badge priority: busy → accent dot; plugin badge (`sidebar.update.badge`)
→ text pill; open windows (views + owned terminals) count when tab not current. Tooltip
`<name> — <n> janela(s) aberta(s)`. Clicking the current tab's icon hides the sidebar.

`sidebar.update(blocks?, rfw?, badge?)`: `badge` key absent → unchanged; `null` or `""` → removed;
other → `"$badge"`. If no blocks and no rfw → just badge; returns `{shown}` (= this plugin's tab is
the live one). Otherwise get-or-create the sidebar tab (`sidebar:<id>`, view id `sidebar`), apply rfw,
then blocks (clearing rfw). Tab layout: open views of the plugin listed on top (`Janelas` ruler) then
the plugin content in **bare** mode (font one step smaller, tighter padding). Before the first
update: spinner `Abrindo <name>…`, or if crashed `<name> caiu: <crash>` + `Reiniciar`.

Own screen (`contributes.screen`): see docs; key behaviors:
- Each screen (null = sessions, or plugin id) has its own pane tree + focus; switching parks the
  current one.
- Views of a screen plugin live in its screen; owned terminals too; everything else on sessions.
  `session.openClaude/openShell/focus` (non-owned) bring the user to sessions.
- `releaseView` moves a view to sessions beside the focused pane (`onSessions = true`);
  `returnView` brings it back. `view.open` of a released view goes to sessions.
- Empty screen: plugin glyph, `nada aberto do <name>` and, if `home`, a button with the home
  command title. `home` runs when the icon is clicked and the screen is empty, and at app start with
  that screen in view.
- Views are never restored on app restart; the sessions grid is persisted.

### 4.9 Suggested tasks (`contributes.suggestions`)

The suggestion card shows a switch with the plugin label (first active taker wins); persisted
`suggestionsToPlugin`. With it on, any exit (worktree/local/here) calls
`suggestion.start` (request, 30 s, process started/revived). Failure → sticky banner `<name>: <msg>`
and tracking dropped. The plugin later calls `suggestion.created`; the host then performs the choice
with prompt = `[intro, task.prompt, (worktree) "Você está na branch \`<branch>\`. Leia o código antes de propor o plano."]`
joined by blank lines; worktree branch default `feature/<ref>`, dir default `<ref>`. Unknown/foreign
suggestion id → `null`; suggestion gone → banner `a tarefa <ref> foi criada, mas a sugestão já não estava mais no cartão`, `null`.

---

## 5. Real plugins → features used

Surveyed: `maestria-plugins/{catppuccin,pomodoro,pomodoro_dart,relatorio-do-dia,git,flutter,docker,ssh,argocd,wiboor}`
and `argocd-plugin` (byte-identical to `maestria-plugins/argocd`). `maestria.js` is byte-identical in every node
plugin and in `examples/plugins/exemplo-completo` (md5 `c24a75ccc8094c7e1f93b666ecbd7463`). The README mentions a
`worktrees` plugin that is not in the checkout.

### Consolidated matrix

Notes on the columns:
- "cmds" is the count, then "k" for how many have keys, then "sb" for how many have `"sidebar": true`.
- "sb" means `contributes.sidebar`; "screen" is `contributes.screen.home`; "sugg" is `contributes.suggestions`.

| plugin | process | activationEvents | permissions | cmds | settings | themes | sb | screen | sugg |
|---|---|---|---|---|---|---|---|---|---|
| catppuccin | — | — | — | 0 | 0 | 3 | — | — | — |
| pomodoro | `node main.js` | `onStartup` | `notifications` | 4 / k2 / sb0 (all have `icon`) | 0 | — | yes | — | — |
| pomodoro_dart | `sh run.sh` (Dart AOT binary) | `onStartup` | `notifications` | 4 / k2 / sb0 | 0 | — | yes | — | — |
| relatorio-do-dia | node | `onCommand:relatorio`, `onCommand:hoje` | `hooks` | 2 / k1 / sb0 (icon `receipt`) | 0 | — | yes | — | — |
| git | node | `onCommand:painel`, `repos`, `branch` | `sessions.create` | 3 / k1 / sb1 | 3 | — | yes | — | — |
| flutter | node | 11 × `onCommand:` | none | 11 / k10 / sb1 | 8 (color) | — | yes | — | — |
| docker | node | `onCommand:containers`, `terminal`, `logs`, `compose` | `sessions.create` | 4 / k1 / sb0 | 7 | — | yes | — | — |
| ssh | node | `onCommand:hosts`, `conectar` | `sessions.create` | 2 / k2 / sb1 | 4 | — | yes | `hosts` | — |
| argocd | node | `onCommand:clusters`, `argo`, `navegador`, `kubeconfig` | `sessions.create` | 4 / k1 / sb0 | 14 (incl. select) | — | yes | `clusters` | — |
| wiboor | node | `onCommand:quadro`, `abrir`, `nova`, `branch`, `onSuggestion` | `sessions.create` | 4 / k1 / sb0 | 4 | — | yes | `quadro` | yes |

| plugin | blocks | rfw (file: roots) | floats | modal | form | pick | input | terminals | events | other host methods |
|---|---|---|---|---|---|---|---|---|---|---|
| catppuccin | — | — | — | — | — | — | — | — | — | — |
| pomodoro | — | `float.rfwtxt`: root; `sidebar.rfwtxt`: root | yes, id `timer`, 228×58, `bottomRight`; view fallback | — | — | — | — | — | `sidebar.shown`/`hidden` | `window.notify`, `window.showBanner`, `sidebar.update` (badge null), `float.show`/`update`/`hide`, `view.open`/`update`/`close` |
| pomodoro_dart | — | same files | yes | — | — | — | — | — | same | same |
| relatorio-do-dia | text, calendar, progress, row, button, divider, list | — | — | — | — | — | — | — | `sidebar.*` | `folders.list`, `projects.list`, `sessions.list` (activity), `chats.list`, `pane.openMarkdown`, `view.open`/`update`, `sidebar.update` (badge "…"), banner |
| git | select, text, button, kv, row, input, progress, divider, heading, list, section, console | — | — | — | — | yes | — | plain `openShell` | `sidebar.*` | `folders.list`, `sessions.focused`, `editor.open`, `view.open`/`update`/`close`, `sidebar.update` (badge) |
| flutter | row, select, button, text, input, checkbox, list, section, console | `devtools.rfwtxt`: root | — | — | — | yes | — | — | `sidebar.*` | `folders.list`, `sessions.focused`, `openUrl`, `view.open`/`update`/`appendLines`/`clearLines`, clipboard, `editor.open`, `sidebar.update` (badge) |
| docker | header, tabs, list, section, card, columns, progress, kv, code, text, row, button, input, terminal, console | `sidebar.rfwtxt`: root | — | — | — | yes (also confirm) | — | owned (tags `x:`/`l:`/`r:`/`v:`/`g:`) and embedded (`e:`), plain | `sidebar.*`, `session.closed`/`exited`/`status` | `sessions.list` (owner/embedded/tag/exited), `session.focus`/`close`, `view.*` incl. `appendLines`/`clearLines`, `pane.openMarkdown`, `openUrl`, clipboard, `editor.open`, badge |
| ssh | heading, input, row, select, text, button, divider, list, section | `hosts.rfwtxt`: root | — | — | yes (half, required, showIf, select, danger/primary, error) | yes | yes | owned (tags `s:`/`c:`) | `sidebar.shown`, `session.closed`/`exited`/`status` | `sessions.list` (owner/tag/exited/focused), `session.focus`/`close`, clipboard, `view.open`/`update`/`close`, badge |
| argocd | header, tabs, kv, text, section, list, progress, code, columns, select, row, button, console, terminal, heading | `argo.rfwtxt`: `home` and `tree` | — | yes (`kubesync`) | — | yes (items without label) | yes | embedded (no cwd/tag), inside a modal | `sidebar.shown`, `session.exited` (code), `session.closed` | `view.open`/`update`/`appendLines`/`close`, `openUrl`, banner, badge |
| wiboor | header, section, text, markdown, input, checkbox, select, calendar, button, row, columns, card, tabs, list, progress, divider | `kanban.rfwtxt`: root, needs `rfw: 2` | — | yes (`tarefa.<id>`) | — | yes (value `""`) | — | — (uses `openClaude`) | `sidebar.*`, `session.opened`/`closed`/`status`/`exited` | `sessions.list`, `sessions.focused`, `folders.list`, `session.focus`, `session.openClaude`, `suggestion.created`, `openUrl`, clipboard, `view.*`, badge |

Hook events: no plugin subscribes to `event{type:'hook'}`. relatorio-do-dia declares `hooks` only to get `sessions.list[].activity` and `chats.list`.

Capability flags read from `initialize`:

| plugin | flags read |
|---|---|
| pomodoro, pomodoro_dart | `floats`, `dataDir` (Dart also reads `pluginDir`) |
| docker | `blocks`, `rfw`, `dataDir`, `settings` |
| flutter | `blocks`, `rfw`, `settings` |
| ssh | `rfw`, `input`, `form`, `dataDir`, `settings` |
| argocd | `rfw`, `settings` |
| wiboor | `rfw` (≥2 for drag), `modal`, `dataDir`, `settings` |
| git | `settings` |
| relatorio-do-dia | nothing |

---

### All host methods used by any plugin (deduplicated)

**Notifications from the plugin**
- `log {message}`

**Window**
- `window.showBanner {text, sticky?}`
- `window.notify {title, body}`
- `window.openUrl {url}`
- `window.pick {title, placeholder?, items:[{value, label?, detail?}]}`: returns the value (including `""`) or `null`.
- `window.input {title, placeholder, value}`: returns a string or `null`.
- `window.form {title, fields, buttons, error}`: returns `{action, values}` or `null`.
- `clipboard.write {text}`
- `editor.open {path, line?}`
- `pane.openMarkdown {title, markdown}`

**Sessions and workspace**
- `sessions.list`
- `sessions.focused`
- `folders.list`
- `projects.list`
- `chats.list {day}`
- `session.focus {tabId}`
- `session.close {tabId}`
- `session.openShell {cwd?, command?, label, owned?, tag?, embedded?}`: returns `{tabId}`.
- `session.openClaude {cwd, prompt, label}`: returns `{tabId}`.
- `suggestion.created {suggestionId, ref, intro, branch, dir}`: returns `{tabId, cwd}` or `null`.

**Views**
- `view.open {viewId, title, blocks | rfw{library, root?} + data, modal?}`: returns `{tabId}`, which flutter reads.
- `view.update {viewId, title?, blocks? | data (+rfw?)}`: returns `{open}`.
- `view.appendLines {viewId, id, lines}`: returns `{open}`.
- `view.clearLines {viewId, id}`
- `view.close {viewId}`
- `sidebar.update {blocks? | data + rfw?, badge?}`: returns `{shown}`.

**Floats**
- `float.show {id, width, height, corner, rfw, data}`: returns `{shown}`.
- `float.update {id, data}`: returns `{shown}`.
- `float.hide {id}`

**Documented but never used by any real plugin:** `settings.get`, `terminal.sendText`, `command.busy`, `featuresOrHotfixes.list`, `workspaces.list`.

### Host-to-plugin messages used

- `initialize`: every plugin except relatorio-do-dia handles it explicitly; relatorio-do-dia uses the default.
- `command.invoke {command, context:{cwd, tabId}}`.
- `suggestion.start`: wiboor only.
- `event` types:
  - `sidebar.shown`, `sidebar.hidden`
  - `session.opened`, `session.closed`, `session.exited` (`tabId`, `code`), `session.status`
- `view.action {viewId, action, values}`.
- `settings.changed {settings}`: git, flutter, docker, ssh, argocd, wiboor.
- `shutdown`.

### All block types and fields used (union)

| type | fields used |
|---|---|
| `heading` | `text` |
| `text` | `text`; `style` in `dim`, `faint`, `error`, `warning`, `success`; `align: 'center'` (relatorio); `width` (inside a row) |
| `markdown` | `text` |
| `code` | `text` |
| `divider` | — |
| `section` | `text`; `count` (number or string); `collapsed`; `action`; `style: 'label'` |
| `kv` | `items[{key (may be ''), value (multiline), tone?}]` |
| `progress` | `value?` (float, or int 1); `label`; `detail`; `tone` |
| `list` | `items`, `empty`, `flat`, `alwaysActions` (item fields below) |
| `button` | `action`, `label`, `style` (`primary`, `danger`, `icon`), `icon`, `tone` (may be null), `tooltip`, `disabled`, `menu` (with no `action`) |
| `header` | `title`, `subtitle`, `tone`, `avatar` (`{icon,color}`, `{text,color,shape:'circle'}` or `{svg}`), `status`, `dim`, `wrap`, `actions[{action,icon,tooltip,tone?,disabled?} or {icon,tooltip,menu}]` |
| `card` | `children`, `padding` (int) |
| `tabs` | `items[{label, action, active, count?, icon?}]`, `align: 'center'` |
| `columns` | `children` |
| `row` | `children` (buttons and also input, select, checkbox, text, each with `width`); `align` in `center`, `between`, `end` |
| `terminal` | `tabId`, `expand`, `autofocus`, `empty` |
| `input` | `id`, `label`, `placeholder`, `value`, `multiline`, `submit`, `icon` (`search`, `add`, `pull`), `width` |
| `checkbox` | `id`, `label`, `value`, `action`, `disabled`, `width` |
| `select` | `id`, `label`, `value`, `placeholder`, `options` (strings or `{value,label,branch?}`), `action`, `disabled`, `loading`, `width` |
| `console` | `id`, `lines?` (see below), `max`, `expand`, `height?`, `empty`, `follow: false` |
| `calendar` | `id`, `mode` (`single` or `range`), `value` (`{from,to}`), `min: 'today'`, `max: 'today'`, `action`, `presets[{label,from,to}]` |

`list` item fields:
- `title`, `subtitle`, `subtitleTone`
- `icon`, `tone`, `badge`
- `action`, `actions[{action,icon,tooltip,tone?,label?,verb?}]`
- `menu`: `{action,label,icon,tone?:'red',disabled?,children?:[{action,label,tone:hex|''}]}` or `{type:'divider'}`
- `avatar` (`{svg}`, `{icon,color}`, `{text,shape,color}`, or a name)
- `status`, `dim`, `strong`, `indent` (int), `selected`
- `meta`, `metaTone`
- `expanded`, plus the undocumented `toggle`, which is the action sent when the arrow is clicked
- the unknown key `connect` (ssh), which must be ignored

`console` lines are any of:
- a plain string (including `''`);
- `{text, tone?}`;
- `{text, spans:[{text, tone?, bold?}], tone?}`.

Tones seen across all blocks:
- state names: `green`, `red`, `yellow`, `accent`, `purple`, `faint`, `dim`;
- terminal colours: `blue`, `cyan`, `magenta`, `white`, `black`;
- hex `#RRGGBB`, `''` and `null`.

**`dim` is used as a tone** (flutter's default settings) but is not in the documented tone list.

Icons used that the doc's icon table lacks: `filter`, `layers`, `chevron`, `pull`, plus plugin `.svg` paths such as `icons/close.svg`, `icons/comment.svg`, `icons/attach.svg`, `icons/argo.svg`, `icons/docker.svg`, `icons/wiboor.svg`.

---

### Compatibility rules a reimplemented host must follow (cross-plugin)

1. **Never return `null` for these results:** `view.update` and `view.appendLines` must return `{open: bool}`, `sidebar.update` `{shown: bool}`, and `float.update`/`float.show` `{shown: bool}`. git, docker, ssh and relatorio destructure without guards. pomodoro_dart requires `=== true`.
2. **`view.update` doubles as an existence probe.** docker and wiboor call it first, check `open`, and then call `view.open` again. Re-opening an existing `viewId`, including in a modal, must bring it to the front.
3. **Remember rfw libraries and data per surface.**
   - The library is sent once; later calls carry only `data`, and each top-level key replaces the old one.
   - Sidebars must keep their library across `sidebar.hidden`/`sidebar.shown`.
   - A missing `rfw.root` defaults to `root`.
   - One library can be used with different roots (argocd's `home` and `tree`).
   - `data.theme` must be injected with int ARGB colours plus `mono` as a **font-family string**, and `onAccent`, `blue`, `cyan`, `magenta`.
4. **rfw events:** `event "act" {a:…}` arrives as `view.action{action:'act', values:{a:…}}`.
   - A `Pressable` or `MenuButton` menu choice must arrive with `values.action = <item.action>`.
   - A `DropTarget` drop arrives as `action:'drop'` with `values.payload` and `values.to`.
   - Float events use `viewId = float id`; sidebar events use `viewId = 'sidebar'`.
5. **Ints and doubles in rfw data:**
   - Plugins avoid integral doubles by clamping: pomodoro `progress` stays within 0.0001–0.9999, wiboor `bar.value` within 0.02–0.999, and argocd adds `+0.5`.
   - The host must preserve JSON doubles as doubles and accept ints elsewhere (`flex`, `indent`, `padding`, `progress: 1`).
   - ARGB colour ints exceed 2^31, so the host needs 64-bit ints.
6. **Blocks state rules:**
   - Field values persist by `id` across `view.update`. A new id means an empty field; plugins bump `<gen>` to clear inputs.
   - A `console` re-sent without `lines` keeps its lines; a new console id starts fresh.
   - `values` must include every field in the window, including those nested in `card`, `columns` or `row`, and must always be an object. relatorio reads `values.periodo` unconditionally.
   - Unknown block fields or keys must be ignored, not rejected.
7. **Requests issued after a `command.invoke` or `suggestion.start` reply.**
   - Most plugins return `null` immediately and only then call `window.pick`, `window.form` or `view.open`. Those requests must not be bound to the 30 s command timeout.
   - flutter awaits `window.pick` inside the request instead.
   - `window.pick` must return `""` for an empty-value item, and `null` only on cancel.
   - Items may omit `label`; the host shows `value`.
8. **`sessions.list` fields actually read:**
   - Identity and ownership: `id`, `kind`, `owner`, `tag` (only for the owner), `embedded`.
   - State: `exited`, `focused`, `hibernated`, `status` (the 8 values listed under wiboor), `statusLabel`.
   - Context: `title`, `cwd`, `folder`, `branch`, `project`, `launcher`, `startedAt` (ISO).
   - `activity{prompts, tools, lastPrompt, lastMessage, touched}`, which requires `hooks`.
   - Not read by any plugin: `needsYou`, `onScreen`, `featureOrHotfix`, `workspaces`, `sessionId`.
9. **Session events:**
   - `session.exited {tabId, code}` must fire for both owned and embedded shells. argocd closes its modal only when `code === 0`.
   - Owned shells must survive a plugin restart and be found again through `owner` and `tag`.
   - An embedded shell needs a real PTY, because argocd's script reads `/dev/tty`, and must accept no `cwd`.
10. **Environment variables:** set `MAESTRIA_PLUGIN_DATA` and `MAESTRIA_PLUGIN_ID`. flutter and argocd ignore `initialize.dataDir`, and docker and ssh compare `owner` against the env id.
11. **Process start on `sidebar.shown`:** plugins whose `activationEvents` are only `onCommand:*` (docker, git, flutter, ssh, argocd, relatorio, wiboor) still expect their process to start when the sidebar tab is shown.
12. **Error codes never reach plugins.** Both clients drop the `code`, and only `message` text is seen. docker branches on `/embedded|desconhecid/i`, and the messages surface in banners. Per the docs, permission errors are `-32001`.
13. **Settings:** dotted ids (`colors.app`) are flat keys. `color` values are passed straight through as tones, and the values used include `dim` and `faint`. `settings.changed` sends the full map.
14. **Keys:** pomodoro and pomodoro_dart declare the same `meta+alt+o` and `meta+alt+shift+o`. **NOTE (host code)**: there is no install-order rejection; `commandFor` returns the first matching command in plugin order (alphabetical by dir), so `maestria.pomodoro` wins over `maestria.pomodoro-dart`. Flutter uses bare `f5`, `f6`, `f10` and `f11`.

### 5.x Per-plugin details

#### Repo-level facts

- **argocd copies are identical.** `diff -rq ~/maestria/maestria-plugins/argocd ~/maestria/argocd-plugin` prints nothing, so every file matches, including `bin/` and `ui/`.
- **`maestria.js` is byte-identical everywhere.** All 8 copies (argocd, docker, flutter, git, pomodoro, relatorio-do-dia, ssh, wiboor) plus `~/maestria/argocd-plugin/maestria.js` and `repo/examples/plugins/exemplo-completo/maestria.js` share md5 `c24a75ccc8094c7e1f93b666ecbd7463`.
- **pomodoro_dart shares pomodoro's UI and overlay files.** `ui/float.rfwtxt`, `ui/sidebar.rfwtxt`, `overlay/overlay_linux.py` and `overlay/overlay.swift` are identical to the node pomodoro's.
- **README mentions a plugin that is missing.** `README.md` describes a `worktrees` plugin (`maestria.worktrees`, ⌥⌘W) that does not exist in this checkout. The git log has only 2 commits: "add the plugins and the official plugin catalog" and "add the argocd plugin".
- **`external.json`** is `{"plugins": []}`. `.gitignore` lists `node_modules/`, `.DS_Store` and `dist/`.
- **`scripts/release.mjs`:**
  - Every root folder that has a `maestria-plugin.json` (except `dist` and `scripts`) gets a GitHub release tagged `<folder>-v<version>`, with asset `<folder>-<version>.zip` built by `zip -qr -X <dir> -x '*.DS_Store' '*/node_modules/*'`.
  - It then builds `catalog.json` as `{schema:1, generatedAt, plugins:[…]}` and uploads it to release tag `catalog`. Repo: `$GITHUB_REPOSITORY`, falling back to `MatheusPano/maestria-plugins`.
  - A local entry is `{id,name,version,description,author,maestria,permissions,icon:<raw.githubusercontent URL at tag>|null,url:<release asset URL>,sha256}`.
  - An external entry is `{…summary, icon:null, git, ref, commit}`. The manifest is looked up at the repo root or in its single subfolder.
  - Duplicate ids abort the run. `--dry-run` writes the zips and the catalog to `dist/` only.
- **`.github/workflows/release.yml`:**
  - Triggers: push to `main` touching `*/maestria-plugin.json`, `external.json` or `scripts/release.mjs`; a daily cron `0 9 * * *`; `workflow_dispatch`.
  - Runs on Node 22: `node scripts/release.mjs` with `GH_TOKEN`, permission `contents: write`, concurrency group `release`.

**Host env vars set** (`repo/lib/services/plugins.dart:1180`): `MAESTRIA_API`, `MAESTRIA_PLUGIN_ID`, `MAESTRIA_PLUGIN_DIR`, `MAESTRIA_PLUGIN_DATA`. Plugins depend on them:
- flutter and argocd read only `MAESTRIA_PLUGIN_DATA` and never `initialize.dataDir`.
- docker and ssh compare `sessions.list[].owner` against `MAESTRIA_PLUGIN_ID`.

**Host `initialize` params today** (`plugins.dart:1217`): `apiVersion`, `blocks`, `rfw: 2`, `floats: 1`, `input: 1`, `form: 1`, `modal: 1`, `pluginId`, `pluginDir`, `dataDir`, plus `permissions` and `settings` per the docs.

#### The protocol clients

##### `maestria.js` (Node, identical everywhere)

- **Outgoing ids** are strings: `"p1"`, `"p2"`, … (`` `p${nextId++}` ``). Every message carries `jsonrpc:'2.0'`, one JSON object per line on stdout.
- **`request(method, params)`** resolves with `msg.result`. On an error it rejects with `new Error(msg.error.message)`, so **the error code is dropped**. Plugins only ever see message text, and some match on it (docker matches `/embedded|desconhecid/i`).
- **`notify`** sends with no id. `log(...)` is the notification `log {message}`, with non-strings JSON-stringified and joined by spaces.
- **Incoming requests:**
  - A known method replies `{id, result: <value, or null if undefined>}`.
  - An unknown method replies `{id, error:{code:-32601, message:"método desconhecido: <m>"}}`.
  - A thrown handler replies `{id, error:{code:-32603, message}}`.
- **Incoming notifications:** handler exceptions are logged as `erro em <method>: <stack>`.
- **Responses to the plugin's own requests:** `pending.get(msg.id)` correlates by the same string id. An unknown id is ignored.
- **Defaults:** if no handler is registered, `initialize` returns `{}` and `shutdown` calls `process.exit(0)`.
- **Parsing:** blank lines and lines that are not JSON are ignored. stdin `close` calls `process.exit(0)`.
- **Messages are not serialized.** `dispatch` is not awaited per line, so handlers run concurrently. A plugin can issue requests (`window.pick`, `view.open`) while its own `command.invoke` response is pending or already sent.

##### `pomodoro_dart/maestria.dart`

- **Behaviour matches the JS client:**
  - Ids are `'p${_nextId++}'` strings.
  - `'params': ?params` uses Dart 3.8 null-aware map elements, so `params` is omitted when null.
  - Error replies use `-32601 'método desconhecido: $method'` and `-32603 '$e'`.
  - Notification errors are logged as `erro em $method: $e\n$st`.
- **Errors on its own requests** complete with `RpcError(message)`, where the message is `error['message']` if `error` is a Map, else `"$error"`. The code is dropped here too.
- **Result `null`:** request handlers send `result: null` explicitly when the handler returns null.
- **Params:** `_params` returns `{}` if params is not a Map. Only Map messages are dispatched.
- **Loop:** `runZonedGuarded` logs any uncaught error as `erro: …` instead of crashing. stdin is decoded with `utf8.decoder` and `LineSplitter`. Stream done calls `exit(0)`. Default handlers are `initialize` returning `{}` and `shutdown` calling `exit(0)`.
- **Startup:** `run.sh` runs `cd "$(dirname "$0")"`, then `exec ./build/pomodoro-<os>-<arch>`. `os` is `uname -s` lowercased; `arch` is `x64` or `arm64`. Prebuilt binaries: `darwin-arm64`, `linux-arm64`, `linux-x64`. If the binary is missing it prints `falta o $bin: rode o ./build.sh na pasta do plugin` to stderr and exits 1.
- **`build.sh`** runs `$DART compile exe` (the `--all` flag also cross-compiles linux x64/arm64).

---

### Per-plugin sections

#### 1. catppuccin (`maestria.catppuccin`): themes only, no process

Manifest:
```json
{
  "id": "maestria.catppuccin",
  "name": "Catppuccin",
  "version": "1.0.0",
  "description": "Os temas Catppuccin Mocha, Macchiato e Latte, com o destaque no mauve.",
  "icon": "icons/catppuccin.svg",
  "author": "Matheus Pano",
  "maestria": 1,
  "contributes": {
    "themes": [
      { "path": "themes/mocha.json" },
      { "path": "themes/macchiato.json" },
      { "path": "themes/latte.json" }
    ]
  }
}
```

- **Theme ids:** `catppuccin-mocha` (dark), `catppuccin-macchiato` (dark), `catppuccin-latte` (`"dark": false`).
- **Keys in each theme:** `id`, `label`, `dark`, `canvas`, `bg`, `bgSidebar`, `bgHover`, `bgActive`, `border`, `fg`, `fgDim`, `fgFaint`, `accent`, `green`, `yellow`, `red`, `purple`.
- **`ansi` keys:** `black`, `red`, `green`, `yellow`, `blue`, `magenta`, `cyan`, `white`, `brightBlack`, `brightWhite`. Only those two `bright*` are given, so the host must fall back for the other six.
- **Icon:** a plugin `.svg` (`icons/catppuccin.svg`), with no `main`. The host must render plugin-relative svg icons for a plugin that has no process.

#### 2. pomodoro (`maestria.pomodoro`, node)

Manifest:
```json
{
  "id": "maestria.pomodoro", "name": "Pomodoro", "version": "1.4.0",
  "description": "Um pomodoro num painel pequeno que flutua …",
  "icon": "clock", "author": "Matheus Pano", "maestria": 1,
  "main": ["node", "main.js"],
  "activationEvents": ["onStartup"],
  "permissions": ["notifications"],
  "contributes": {
    "sidebar": true,
    "commands": [
      { "id": "alternar", "title": "pomodoro: iniciar/pausar", "icon": "play", "key": "meta+alt+o" },
      { "id": "painel", "title": "pomodoro: mostrar/esconder o painel", "icon": "open", "key": "meta+alt+shift+o" },
      { "id": "pular", "title": "pomodoro: pular pra próxima fase", "icon": "step-over" },
      { "id": "zerar", "title": "pomodoro: zerar o ciclo", "icon": "restart" }
    ]
  }
}
```
The icon is a built-in name (`clock`). Commands have an `icon` but no `"sidebar": true`, so there are no footer buttons.

**Host methods called:**
- `window.notify {title:'Pomodoro', body}`: result ignored, errors logged.
- `window.showBanner {text}`.
- `sidebar.update`, in three forms:
  - `{data, rfw?:{library}, badge?:null}`: `rfw` is sent only the first time (`env.sideSent`). There is no `root`, so the host must default it to `root`.
  - `{badge:null}` alone, once, to clear an old badge.
  - The result is ignored.
- `float.show {id:'timer', width:228, height:58, corner:'bottomRight', rfw:{library}, data}`.
- `float.update {id:'timer', data}`: reads `r.shown`. A falsy result triggers a `float.show` again.
- `float.hide {id:'timer'}`.
- Fallback when `floats` is missing:
  - `view.open {viewId:'timer', title:'pomodoro', rfw:{library}, data}`.
  - `view.update {viewId:'timer', data}`, reading `.open` (falsy means the window was closed and the panel is treated as hidden).
  - `view.close {viewId:'timer'}`.
- `log`.

**Host messages handled:**
- **`initialize`:** reads `p.floats >= 1` and `p.dataDir`, and returns `{}`. Rendering starts only after the reply (`setImmediate`), because before that the host would not accept calls.
- **`command.invoke`:** reads `{command}` with ids `alternar`, `painel`, `pular`, `zerar`. An unknown id falls through to `act()` and is a no-op. Returns `null`.
- **`event`:** only `sidebar.shown` and `sidebar.hidden`.
- **`view.action`:**
  - Accepts only `viewId === 'timer'` (the float id) or `'sidebar'`.
  - The action is resolved as `values.action || values.a || action`, and the id comes from `values.id`.
  - Verbs: `alternar`, `pular`, `zerar`, `esconder`, `fase` with id, `fase:<focus|short|long>`, `mais`/`menos` with id, `preset` with id (`25-5`, `50-10`, `90-20`), `cena` with id, `icone` with id, `cor` with id, `testar`, `toggle` with id (`visible`, `autoStart`, `notify`, `overlay`, `sound`, `confetti`), and `nada` (the disabled menu item).
- **`shutdown`:** saves state, then exits.
- `settings.changed` is not handled, and the manifest has no settings.

**UI:** rfw only, no blocks.

`ui/float.rfwtxt`, root `root` (inner widget `Ring`):
- Data: `phase`, `running`, `clock`, `label`, `progress`, `color`, plus `menu`.
- Menu items are `{label, icon, action, disabled}` or `{divider:true}`.
- Events: `event "act" {a:"alternar"|"pular"}`, and `MenuButton(icon:"more", menu: data.menu, onMenu: event "act" {})`. The host must add the chosen `action` into the values.

`ui/sidebar.rfwtxt`, root `root`:
- Local widgets: `Segmented`, `Dial`, `Controls`, `Today`, `Ruler`, `Preset`, `Stepper`, `Toggle`, `Caption`, `IconChip`, `Swatch`.
- Data (everything above, plus): `today`, `dots[bool]`, `focusColor`, `phases[{id,label,active}]`, `presets[{id,label,active}]`, `steppers[{id,label,value,atMin,atMax}]`, `toggles[{id,label,hint:[str],on}]`, `panel[{…same}]`.
- `anim:{phases, icons[{id,label,emoji,active}], colors[{id,label,value:int ARGB,active}], color, toggles, test}`.
- Events: `event "act" {a: …, id: …}`.

Widgets:
- Maestria: `Glyph`, `IconButton` (with `disabled`, `size`, `extent`), `MenuButton`, `Pressable`, `Tooltip`.
- Core and material: `CircularProgressIndicator` (`value`, `strokeWidth`, `color`, `backgroundColor`), `ListView`, `Wrap`, `Align`, `Stack`, `Center`, `Container` and similar.

Theme keys used: `fg`, `dim`, `faint`, `active`, `canvas`, `hover`, `accent`, `onAccent`, `border`, and `mono`. **`mono` is used as `fontFamily`, so the host must send it as a string.**

**Env, files and binaries:**
- `MAESTRIA_PLUGIN_DATA`, overridden by `initialize.dataDir`. State lives in `<data>/state.json`: `{phase, running, endsAt (epoch ms), left, round, today:{day,count}, visible, config:{focus,short,long,longEvery,autoStart,notify,sound,overlay,confetti,scenes}}`.
- On macOS, `<data>/overlay-<sha1[0:10]>` is compiled with `swiftc -O overlay/overlay.swift`. Linux runs `python3 overlay/overlay_linux.py` after probing python3-gi with cairo, with `GDK_BACKEND=x11` when `DISPLAY` is set.
- Sound comes from `afplay /System/Library/Sounds/Glass.aiff` on macOS. On Linux it tries `pw-play`, then `paplay`, then `canberra-gtk-play -i complete`, playing `/usr/share/sounds/freedesktop/stereo/complete.oga`.

**Quirks:**
- `progress` is clamped to the range 0.0001–0.9999 on purpose, so JSON never sends an int 0 or 1 into `CircularProgressIndicator.value`. Ints there would make the ring spin as indeterminate.
- Colours are ints `0xFF000000 + rgb`, which exceed 2^31, so the host must use 64-bit ints.
- The float library is resent on every `float.show` after `shown:false`.
- The plugin expects the float to vanish when the plugin stops, and calls `float.show` again on start.

#### 3. pomodoro_dart (`maestria.pomodoro-dart`, a native Dart binary)

Manifest: the same as pomodoro, except:
- `"id": "maestria.pomodoro-dart"`, `"name": "Pomodoro (Dart)"`, `"version": "1.4.0"`, `"main": ["sh", "run.sh"]`.
- The description is different.
- The commands, keys, permissions, `activationEvents` and the `sidebar` contribution are identical, so the two plugins' keys collide.

Behaviour is a port of `pomodoro/main.js`, with the same methods, params, data keys and actions. The differences:
- `initialize` also reads `pluginDir`, defaulting to the cwd, and loads `ui/*.rfwtxt` from it. Its type checks are stricter: `floats` is `p['floats'] is num ? … : 0`.
- `view.update` is treated as open only if `r is Map && r['open'] == true`. `float.update` is treated as shown only if `r['shown'] == true`. **The host must return real booleans.**
- The overlay binary is named with an FNV-1a 64-bit hash (`overlay-<10 hex>`) instead of sha1.
- State JSON is parsed field by field. `endsAt`, `left` and `round` are read as `num` and converted with `toInt()`.
- `GDK_BACKEND=x11` is passed as `environment` and merged with the parent env.

#### 4. relatorio-do-dia (`maestria.relatorio-do-dia`)

Manifest:
```json
{
  "id": "maestria.relatorio-do-dia", "name": "Relatório do dia", "version": "1.2.0",
  "description": "Os commits, o trabalho sem commit e as sessões de um dia ou de um período, contados pelo claude como uma daily.",
  "icon": "report", "author": "Matheus Pano", "maestria": 1,
  "main": ["node", "main.js"],
  "activationEvents": ["onCommand:relatorio", "onCommand:hoje"],
  "permissions": ["hooks"],
  "contributes": {
    "sidebar": true,
    "commands": [
      { "id": "relatorio", "title": "relatório do dia…", "key": "meta+shift+r", "icon": "receipt" },
      { "id": "hoje", "title": "relatório de hoje" }
    ]
  }
}
```

**Host methods called:**
- `folders.list`: reads `name`, `root`, `isRepo`, `branch`, and `worktrees[]` (`prunable`, `isMain`, `label`, `branch`, `path`).
- `projects.list`, the deprecated alias: reads `name`, `folder`, `brief`.
- `sessions.list`, which reads:
  - Filters: `kind` (keeps `claude`/`shell`), `startedAt` (ISO-parsable), `folder`.
  - Descriptive fields: `title`, `launcher`, `project`, `branch` (`'(detached)'` is special), `exited`, `statusLabel`.
  - `activity.{prompts, tools, lastPrompt, lastMessage, touched[]}`, which requires the `hooks` permission.
- `chats.list {day:'YYYY-MM-DD'}`: one call per past day in the range. Reads `title`, `folder`, `at`, `size`, and sorts by `at`.
- `window.showBanner {text, sticky?}`.
- `pane.openMarkdown {title, markdown}`.
- `view.open {viewId:'relatorio', title:'relatório do dia', blocks}`.
- `view.update {viewId:'relatorio', blocks}`: reads `r.open` with no guard.
- `sidebar.update`, in two forms:
  - `{blocks, badge: '…'|null}` when shown: reads `r.shown` with no guard.
  - `{badge}` when hidden.

**Host messages handled:**
- `initialize`: the default (returns `{}`).
- `command.invoke`: `relatorio` opens the view. `hoje` writes without awaiting. Any other id throws `comando desconhecido: <cmd>`.
- `event`: `sidebar.shown` and `sidebar.hidden`. Both trigger a refresh.
- `view.action`:
  - Accepts `viewId` `relatorio` or `sidebar`.
  - It always reads `values.periodo`, so **`values` must be present.**
  - Actions: `periodo`, `escrever`, `material`, `abrir`.

**Blocks used:**
- `text {style:'dim', align:'center', text}`: `align` on a `text` block is not in the type's field table.
- `calendar {id:'periodo', mode:'range', value:{from,to}, max:'today', action:'periodo', presets:[{label,from,to}]}`: the returned value must be `values.periodo = {from,to}`.
- `progress {label}`: indeterminate.
- `row {align:'center', children:[button…]}`.
- `button {action, label, icon:'report'|'file', style?:'primary'}`.
- `divider`.
- `list {items:[{title, subtitle, icon:'check'|'warning', tone:'green'|'yellow', action:'abrir'}]}`.

**External:**
- `git log --all --source --no-merges --since/--until --date=format:… --pretty=format:%h§%ad§%S§%s [--author=email]`, plus `git config user.email` and `git status --porcelain`.
- `/bin/zsh -lc "claude -p --output-format text < '<tmp>/prompt.md'"`, run in `$HOME`, with the timeout given by `TIMEOUT_MS`.
- No data files.

**Quirk:** the badge is the string `'…'` while busy and `null` otherwise. A 1 s ticker re-sends `view.update` and `sidebar.update`.

#### 5. git (`maestria.git`)

Manifest:
```json
{
  "id": "maestria.git", "name": "Git", "version": "1.2.1",
  "description": "O controle de código do VS Code numa janela: …",
  "icon": "icons/git.svg", "author": "Matheus Pano", "maestria": 1,
  "main": ["node", "main.js"],
  "activationEvents": ["onCommand:painel", "onCommand:repos", "onCommand:branch"],
  "permissions": ["sessions.create"],
  "contributes": {
    "sidebar": true,
    "commands": [
      { "id": "painel", "title": "git: mudanças…", "key": "meta+shift+g" },
      { "id": "repos", "title": "git: escolher repositório…", "icon": "icons/git.svg", "sidebar": true },
      { "id": "branch", "title": "git: trocar de branch…" }
    ],
    "settings": [
      { "id": "smartCommit", "type": "boolean", "default": true, "title": "commit sem nada preparado leva tudo", "description": "…" },
      { "id": "poll", "type": "number", "default": 2, "title": "conferir mudanças a cada (segundos)", "description": "…" },
      { "id": "commits", "type": "number", "default": 15, "title": "commits recentes mostrados" }
    ]
  }
}
```

**Host methods called:**
- `folders.list`: reads `isRepo`, `root`, `name`, `branch`, and `worktrees[]` (`isMain`, `prunable`, `path`, `label`, `branch`).
- `sessions.focused`: reads `.cwd`.
- `window.pick`, used three ways:
  - Repo picker: `{title:'git de qual repositório?', placeholder:'nome da pasta', items:[{value,label,detail}]}`.
  - Discard confirmation, with no placeholder: items `{value:'nao',label:'não, deixa'}` and `{value:'sim',label,detail:'não dá pra desfazer'}`.
  - Branch picker: values `'+'`, `'l:<name>'`, `'r:<name>'`.
- `window.showBanner {text, sticky?}`.
- `view.open {viewId:'git'|'diff', title, blocks}`.
- `view.update`: destructured as `{open}`, so the host must return an object.
- `view.close {viewId:'diff'}`.
- `sidebar.update`, in two forms:
  - `{blocks, badge}`, destructured as `{shown}`.
  - `{badge}` alone. The badge is `''` when clean, otherwise the changed-file count, capped at `'99+'`.
- `editor.open {path}`.
- `session.openShell {cwd, label}`: a plain, non-owned shell. On error it shows the banner `'abrir terminal pede a permissão sessions.create'`.
- `log`.

**Host messages handled:**
- `initialize`: reads `settings` only and returns `{}`.
- `settings.changed {settings}`.
- `command.invoke`: `painel`, `repos`, `branch`, reading `context.cwd`. It returns `null` immediately and does the work without awaiting. Unknown ids throw `comando desconhecido: <cmd>`.
- `event`: `sidebar.shown` (reload and start polling) and `sidebar.hidden`.
- `view.action`:
  - Accepts viewIds `git`, `diff`, `sidebar`.
  - Plain actions: `repo`, `atualizar`, `commit`, `sugerir`, `fetch`, `pull`, `push`, `terminal`, `branch`, `criar-branch`, `cancelar-branch`, `preparar-tudo`, `tirar-tudo`, `painel`, `c:<hash>`.
  - File actions: `f|o|a|r|d:<w|s|x>:<path>`, where the path may contain `:`.
  - Values read: `repo`, `msg.<gen>`, `nb.<gen>`.
- `shutdown`: the client default.

**Blocks** (no rfw):
- `select {id:'repo', label?, options:[{value,label,branch}], value, action:'repo'}`: options carry an extra `branch` key.
- `text {style: error|faint|dim|warning, text}`.
- `button`, three forms:
  - `{action, label, icon, tooltip, disabled, style: primary|danger}`.
  - Icon-only `{action, icon, tooltip, disabled}`, sometimes with `style:'icon'`.
  - Without `style`, but with an icon and no label.
- `kv {items:[{key,value,tone}]}`.
- `row {children}`.
- `input {id, label?, placeholder, value?, submit}`.
- `progress {label}`.
- `divider`.
- `heading {text}`.
- `list {items, empty?, flat?}`: item fields `title`, `subtitle`, `icon`, `tone`, `badge`, `action`, `actions:[{action,icon,label,tooltip,verb}]` (`label` and `verb` are extra keys).
- `section {text, count?}`.
- `console {id:'d.<gen>', lines, max, expand:true, follow:false, empty}`: lines can be strings (including `''`) or `{text, tone}`.

**External:**
- `git` with `GIT_TERMINAL_PROMPT=0` and `GIT_OPTIONAL_LOCKS=0`: `status --porcelain=v2 --branch -z`, `add`, `restore`, `clean`, `commit`, `fetch --prune`, `pull`, `push [-u]`, `switch`, `check-ref-format`, `show`, `remote`, `log`.
- `/bin/zsh -lc "claude -p --output-format text < tmp"`, used for the commit-message suggestion (3 min).
- No data files.

**Quirks:**
- Both `view.update` and `sidebar.update` results are destructured, so neither may return null.
- Inputs are cleared by bumping the id.
- `values` must include every field, from the sidebar as well as windows.
- Unknown keys must be ignored.

#### 6. flutter (`maestria.flutter`)

Manifest fields:
- `id: "maestria.flutter"`, `name: "Flutter"`, `version: "1.5.0"`, `icon: "icons/flutter.svg"`, `author: "Matheus Pano"`, `maestria: 1`, `main: ["node","main.js"]`.
- `activationEvents`: `onCommand:central`, `onCommand:iniciar`, `onCommand:parar`, `onCommand:reload`, `onCommand:restart`, `onCommand:pausar`, `onCommand:over`, `onCommand:into`, `onCommand:out`, `onCommand:devtools`, `onCommand:devtools-navegador`.
- No `permissions`.
- `contributes.sidebar: true`.

Commands (11):

| id | title | key | other |
|---|---|---|---|
| `central` | "flutter: central de debug…" | `meta+shift+d` | `icon: "icons/flutter.svg"`, `sidebar: true` |
| `iniciar` | "flutter: iniciar / continuar" | `f5` | |
| `parar` | "flutter: parar" | `shift+f5` | |
| `reload` | "flutter: hot reload" | `meta+alt+r` | |
| `restart` | "flutter: hot restart" | `meta+shift+f5` | |
| `pausar` | "flutter: pausar" | `f6` | |
| `over` | "flutter: step over" | `f10` | |
| `into` | "flutter: step into" | `f11` | |
| `out` | "flutter: step out" | `shift+f11` | |
| `devtools` | "flutter: DevTools (desempenho, CPU, memória e rede)" | `meta+alt+t` | |
| `devtools-navegador` | "flutter: abrir o DevTools no navegador" | — | |

Settings (8), all `type:"color"`, `group:"cores do console"`:

| id | default |
|---|---|
| `colors.app` | `""` |
| `colors.developer` | `""` |
| `colors.native` | `"dim"` |
| `colors.warning` | `"yellow"` |
| `colors.error` | `"red"` |
| `colors.success` | `"green"` |
| `colors.tool` | `"accent"` |
| `colors.build` | `"faint"` |

The keys are dotted but flat in the settings object.

**Host methods called:**
- `folders.list`: reads `root`, and `worktrees[]` (`prunable`, `isMain`, `label`, `branch`, `path`).
- `sessions.focused`: reads `.cwd`.
- `window.showBanner {text, sticky?}`: `sticky:false` is sent explicitly.
- `window.pick {title, placeholder, items:[{value,label,detail}]}`: titles `'abrir a central de debug em qual projeto?'` and `'abrir qual emulador?'`. These calls are awaited inside `command.invoke`, so they run under the 30 s limit.
- `window.openUrl {url}`.
- `view.open`: reads `r.tabId`.
  - Central: `{viewId:'central-<sha1(root)[0:10]>', title, blocks}`.
  - DevTools: `{viewId:'devtools-<…>', title, rfw:{library, root:'root'}, data}`.
- `view.update`:
  - Central `{viewId,title,blocks}`: reads `r.open` with no null check.
  - DevTools `{viewId,title,data}`: checks `r.open === false`.
- `view.appendLines {viewId, id:'console', lines}`: reads `.open`.
- `view.clearLines {viewId, id:'console'}`.
- `clipboard.write {text}`.
- `editor.open {path, line?}`.
- `sidebar.update`, in two forms:
  - `{blocks, badge}`: reads `r.shown === false`.
  - `{badge}` alone when hidden and the badge changed.
  - Badges: `null`, `'▶'`, `'⏸'`, or a count string.

**Host messages handled:**
- **`initialize`:** reads `settings`, `rfw` (truthy), and `blocks` via `Number()||1`. Returns `{}`.
  - Without `rfw`, DevTools opens in the browser with the banner `'esta Maestria não desenha widgets de plugin — abrindo o DevTools no navegador'`.
  - With `blocks >= 2`, the sidebar shows worktrees as children using `expanded` and `toggle`.
- **`settings.changed`:** re-renders consoles with all their lines.
- **`command.invoke`:** the 11 ids above. Reads `context.cwd` and `context.tabId`, matching the tabId to a central's `view.open` tabId. Unknown ids throw.
- **`event`:** `sidebar.shown` calls `sessions.focused`, then `folders.list`, then pushes the sidebar. `sidebar.hidden` is handled.
- **`view.action`, `sidebar`:** `abrir:<key>`, `iniciar:<key>`, `parar:<key>`, `recolher:<key>`, `central`, `config`, `device`, and toolbar verbs. `<key>` is sha1(root)[0:10].
- **`view.action`, central:**
  - Verbs: `config`, `device`, `preferencias`, `iniciar`, `aparelhos`, `emulador`, `continuar`, `pausar`, `over`, `into`, `out`, `reload`, `restart`, `parar`, `devtools`, `filtrar`, `limpar`, `copiar`, `sdk`, `frame:<i>`.
  - Values read: `config`, `device`, `autoReload`, `pauseExceptions`, `filtro`, `sdk`.
- **`view.action`, DevTools:**
  - The action is resolved as `values.a || values.action || action`.
  - Verbs: `tab:<perf|cpu|mem|net>`, `navegador`, `frame:<n>`, `gravar-frames`, `limpar-frames`, `toggle:<ext>`, `cpu`, `cpu-filtro:<x>`, `cpu-ordem:<self|total>`, `fn:<i>`, `classes`, `gc`, `marcar`, `classes-filtro:<all|app>`, `classe:<i>`, `req:<id>`, `rede-fechar`, `rede-pausar`, `rede-limpar`, `rede-curl`, `copiar:<i>`.
- **`shutdown`:** kills the `flutter run` process groups and devtools, then exits. It also does this on SIGTERM and on exit.

**Blocks:**
- `row {children}`: non-button children carry a `width` (select 280/220, checkbox 180/240, input 200/420).
- `select {id, width?, options:[{value,label}], value, action, disabled, loading}`.
- `button`: `{style:'icon', action, icon, tooltip, tone (can be null), disabled}` and `{action:'sdk', label}`.
- `text {style: error|warning|dim|success|faint}`.
- `input {id, width, placeholder, value, submit}`.
- `checkbox {id, width, label, value, action}`.
- `list`:
  - Stack frames: `items:[{title,subtitle,icon,tone,action}]`.
  - Sidebar: `{flat:true, items}` with item fields `title`, `subtitle`, `icon`, `tone`, `indent`, `badge:'ativo'`, `action`, `actions:[{action,icon,tooltip,tone}]`, `expanded`, `toggle:'recolher:<key>'`.
- `section {text, count?}`.
- `console {id:'console', expand:true, empty, max:5000, lines?}`:
  - Re-renders omit `lines`, and the existing lines must be kept.
  - Lines are `{text, tone?}`, where the tone is the raw setting value: hex, `''`, or a theme name including `dim` and `faint`.

**rfw** (`ui/devtools.rfwtxt`, root `root`):
- Widgets:
  - Maestria: `Glyph` (including `"icons/close.svg"`), `IconButton` (`size`, `extent`), `Tooltip`, `Pressable`.
  - Core: `GestureDetector(behavior:"opaque")`, `ListView(scrollDirection:"horizontal", reverse, itemExtent)`, `Positioned(start/end/top/bottom)`, `Expanded(flex)`, `Flexible`, `Wrap`, `Stack`.
- Data keys:
  - `head{live,tab,status,dot,offline,tabs[{label,icon,action,active}]}`
  - `perf{bars[{b,bt,bc,r,rt,rc,tip,action,sel}],detail,hint,recording,scaleTop,scaleMid,stats,toggles,togglesNote,warn,worst,worstEmpty}`
  - `cpu{button,buttonIcon,empty,filters,primary,rows,sorts,status,warn}`
  - `mem{classes,deltaHead,filters,from,mark,marked,markIcon,note,samples[{empty,free,ext,used,gc,tip}],scale,stats}`
  - `net{status,paused,empty,rows[{method,status,tone,path,host,time,size,action,sel}],open,detail{title,kv[{k,v,tone}],sections[{title,text,action}]}}`
- Theme keys: `accent`, `active`, `bg`, `blue`, `border`, `canvas`, `cyan`, `dim`, `faint`, `fg`, `green`, `hover`, `mono` (a font family string), `onAccent`, `red`, `sidebar`, `yellow`.
- Bar heights are passed as int `flex` values (1–100 and 0–1000) to avoid the double problem.

**Env and files:**
- `MAESTRIA_PLUGIN_DATA`, with fallback `__dirname`. `initialize.dataDir` is not used.
- Files: `lateral.json` `{collapsed:[root]}` and `projetos.json` `{[root]:{config,device,autoReload,pauseExceptions,sdkOverride}}`.
- Also reads `HOME` and `PATH`.

**Binaries:**
- `flutter run --machine`, spawned detached and killed by process group.
- `flutter devices --machine`, `flutter daemon`, `flutter emulators --launch <id> [--cold]`.
- `dart devtools --machine --no-launch-browser`, `fvm api context`.
- The VM service is reached over Node 22's global `WebSocket`.
- Reads `.vscode/launch.json` (JSONC), `.dart_tool/package_config.json` and `pubspec.yaml`, and runs `fs.watch` on `lib/` recursively for hot reload on save.

#### 7. docker (`maestria.docker`)

Manifest fields:
- `id "maestria.docker"`, `name "Docker"`, `version "1.1.0"`, `icon "icons/docker.svg"`, `author "Matheus Pano"`, `maestria 1`, `main ["node","main.js"]`.
- `activationEvents`: `onCommand:containers`, `onCommand:terminal`, `onCommand:logs`, `onCommand:compose`.
- `permissions ["sessions.create"]`.
- `contributes.sidebar: true`.

Commands:
- `containers`, "docker: containers, imagens e volumes…", key `meta+alt+c`
- `terminal`, "docker: terminal num container…"
- `logs`, "docker: logs de um container…"
- `compose`, "docker: projetos do compose…"

Settings:

| id | type | default | group |
|---|---|---|---|
| `stopped` | boolean | true | — |
| `stats` | boolean | true | — |
| `tail` | number | 500 | `logs` |
| `max` | number | 5000 | `logs` |
| `timestamps` | boolean | false | `logs` |
| `shell` | string | "" | — |
| `binary` | string | "docker" | — |

**Host methods called:**
- **`sessions.list`:** reads `id`, `owner`, `embedded`, `title`, `tag`, `exited`. It keeps sessions where `owner === MAESTRIA_PLUGIN_ID` and that are not embedded.
- **`session.openShell`**, in three forms:
  - Owned: `{cwd: homedir, command, label, owned:true, tag}`. Tags: `x:<id>` (exec), `l:<project>` (compose logs), `r:<image>` (run), `v:<volume>`, `g:<id>` (logs).
  - Embedded: `{cwd, command, label, embedded:true, tag:'e:<id>'}`.
  - Plain: `{cwd: workdir, label}`.
  - Reads `{tabId}`, tolerating null. On an error matching `/embedded|desconhecid/i` it shows "esta Maestria ainda não embute terminais — atualize o app".
- `session.focus {tabId}`, `session.close {tabId}`.
- **`sidebar.update`**, in three forms; it reads `{shown}`:
  - `{data, badge, rfw?:{library, root:'root'}}`: the library is sent once per process and never again.
  - `{blocks, badge}`.
  - `{badge}`.
  - The badge is the running count string or `''`.
- **Views:**
  - `view.open {viewId,title,blocks}`.
  - `view.update {viewId,title,blocks}`: destructured `{open}`. It is used as a probe ("is it open?") before calling `view.open` again.
  - `view.appendLines {viewId, id:'log.<gen>', lines}`: reads `{open}`.
  - `view.clearLines`, `view.close`.
- **`window.pick {title, placeholder?, items}`:** also serves as the confirm dialog (`nao`/`sim`).
- **Other:** `window.showBanner {text,sticky?}`, `window.openUrl`, `clipboard.write`, `editor.open {path}`, `pane.openMarkdown {title:'inspect · <name>', markdown}`, `log`.

ViewIds: `docker` (main), `ct.<12-char id>` (container), `pj.<project>` (compose project), `sidebar`.

**Host messages handled:**
- **`initialize`:** reads `dataDir`, `settings`, `blocks` (`Number()||1`; below 2 means legacy UI), and `rfw >= 1` (the sidebar is drawn in rfw). Returns `{}`. Then it starts `docker events` and pushes the badge, so the badge shows at startup.
- **`command.invoke`:** the 4 ids. Returns `null` at once and runs the work unawaited. Unknown ids throw.
- **`event`:** `sidebar.shown`, `sidebar.hidden`, `session.closed {tabId}`, `session.exited {tabId}` (marks the embedded shell dead), `session.status {tabId}`.
- **`view.action`:** if `action==='act'`, the real action is `values.a || values.action`. Actions are `verb[:arg]`.
  - Everywhere:
    - General: `atualizar`, `buscar`, `filtrar` (reads `values.filtro`), `recolher:<key>`, `parados`, `janela`, `ligar`, `contexto:<name>`, `limpar-parados`.
    - Containers: `abrir:<id>`, `terminal:<id>`, `terminal-painel:<id>`, `terminal-pick`, `portas:<id>`, `iniciar|parar|reiniciar|pausar|retomar|remover:<id>`.
    - Projects: `projeto-abrir:<p>`, `projeto:<p>`, `projetos`, `p-subir|p-parar|p-reiniciar|p-derrubar|p-logs|p-pasta|p-editor:<p>`.
    - Owned terminals: `focar:<tabId>`, `fechar:<tabId>`.
    - Misc: `cor:<p|n>:<name>:<tone>`, `url:<url>`, `copiar:<text>`.
  - Main window:
    - Tabs: `aba:containers|imagens|volumes`.
    - Pull: `puxar` (reads `values['pull.<gen>']`).
    - Cleanup: `limpar-imagens`, `limpar-imagens-todas`, `limpar-build`, `limpar-volumes`, `limpar-volumes-todos`.
    - Images and volumes: `rmi:<id>`, `rmv:<name>`, `rodar:<ref>`, `vol-terminal:<name>`.
  - Container window: `aba:info|stats|logs|terminal`, `terminal`, `reabrir-terminal`, `navegador`, `inspecionar`, `copiar-id`, `limpar`, `logs-terminal`.
  - Project window: `aba:info|logs`, `finder`, `limpar`.
- **`settings.changed`:** `binary` restarts events; `stats` toggles `docker stats`.
- **`shutdown`:** the default (exit). Child processes are killed on exit.

**Blocks** (when `blocks >= 2`):
- `header {title, subtitle, tone, status, dim, avatar:{icon,color}|{svg}, actions:[{action,icon,tooltip,tone?,disabled?} | {icon:'more',tooltip,menu}]}`.
- `tabs {items:[{label,icon?,count?,action,active}], align:'center'}`.
- `list {flat, alwaysActions, empty, items}`:
  - Basic item fields: `title`, `subtitle`, `icon`, `tone`, `badge`, `action`, `selected`, `dim`, `strong`, `indent` (0/1), `status`, `meta`.
  - `avatar:{svg}|{icon,color}`, `expanded`, `toggle:'recolher:p:<proj>'`, `actions:[{action,icon,tooltip}]`.
  - `menu:[{action,label,icon,tone:'red',disabled} | {type:'divider'} | {label:'Cor',icon:'dot',children:[{action,label,tone:'#RRGGBB'|''}]}]`.
- `section`: `{text,count}`, `{style:'label',text}`, and the legacy `{text,count,collapsed,action}`. `count` can be a number or a string like `"2/5"`.
- `card {children, padding?}`, `columns {children}`.
- `progress`: `{label}`, or `{label,detail,value,tone}`.
- `kv {items:[{key,value,tone?}]}`: `key` may be empty.
- `code {text}`.
- `text {style: warning|error|faint, width?}`.
- `row {children}`: mixed children such as an input with `width:340` and text with `width:120`.
- `button {action,label,style:'primary',icon,disabled}`, and the menu form `{label:'limpar', icon:'trash', menu}` with no action.
- `input {id:'filtro'|'pull.<gen>', icon:'search'|'pull', placeholder, value, submit, width?}`.
- `terminal {tabId, expand:true, autofocus, empty:'abrindo o shell…'}`.
- `console {id:'log.<gen>', max, expand:true, empty, lines?}`:
  - Lines are `{text,tone}` or `{text, spans:[{text,tone?,bold?}]}`.
  - Span tones are ANSI names (`black` becomes `faint`), hex, or `faint`.

**rfw sidebar** (`ui/sidebar.rfwtxt`, root `root`):
- Local widgets: `Head`, `Label`, `Avatar`, `TitleText`, `SubText`, `Actions`, `Pill`, `ContainerRow`, `ProjectRow`, `TermRow`.
- Maestria widgets: `Svg`, `Glyph(icon:"chevron"|"terminal")`, `IconButton(icon:"refresh"|"layers"|…)`, `MenuButton(icon:"filter")`, `Pressable(menu,onMenu,padding)`.
- Core widgets: `Rotation(turns,duration:140)`, `Positioned(end,bottom)`, `Opacity`.
- Data:
  - `head:{title:'Containers', sub, menu}`.
  - `rows:[{kind:'label',title} | {kind:'term',title,sub,dot,action,actions} | {kind:'container'|'project', title,sub,svg,dot,dim,selected,strong,indent(int),open,toggle,action,pill,actions[{icon,tooltip,action}],menu}]`.
  - The rfw menu format is `{label,icon,action,red,disabled,children?:[{label,action,color:int}]} | {divider:true}`.
- Theme keys: `fg`, `dim`, `faint`, `green`, `red`, `yellow`, `accent`, `onAccent`, `sidebar`, `hover`, `active`.

**Env, files and binaries:**
- `MAESTRIA_PLUGIN_ID` and `MAESTRIA_PLUGIN_DATA`. State lives in `<data>/state.json` `{context, expanded, showStopped, colors}`.
- Child env adds `DOCKER_CLI_HINTS=false` and `LC_ALL=C`.
- Every call is `docker [--context X] …`:
  - Reads: `ps -a --no-trunc --format '{{json .}}'`, `images`, `volume ls`, `system df`, `context ls`, `version`, `info`, `inspect`.
  - Streams: `events --format '{{json .}}' --filter type=container|image|volume`, `logs -f --timestamps`, `stats --format '{{json .}}'`.
  - Container actions: `start`, `stop`, `restart`, `pause`, `unpause`, `rm`.
  - Compose: `compose -p … up -d|stop|restart|down|logs -f`.
  - Images, volumes, cleanup: `pull`, `rmi`, `volume rm`, `* prune -f`.
  - In terminals: `exec -it`, `run --rm -it`.
- Starting an engine: `orb start`, `colima start`, `open -a Docker`.

**Quirk:** the manifest has only `onCommand:*` activation events, so the sidebar works only because the host starts the process on `sidebar.shown`.

#### 8. ssh (`maestria.ssh`)

Manifest fields:
- `id "maestria.ssh"`, `name "SSH"`, `version "1.3.0"`, `icon "icons/server.svg"`, `author "Matheus Pano"`, `maestria 1`, `main ["node","main.js"]`.
- `activationEvents`: `onCommand:hosts`, `onCommand:conectar`.
- `permissions ["sessions.create"]`.
- `contributes.sidebar: true`, `contributes.screen: {"home":"hosts"}`.

Commands:
- `hosts`, "ssh: hosts…", key `meta+alt+s`, `icon "icons/server.svg"`, `sidebar: true`
- `conectar`, "ssh: conectar a um host…", key `meta+shift+s`

Settings:
- `sshConfig`, boolean, default true
- `binary`, string, default "ssh"
- `timeout`, number, default 10
- `extra`, string, default ""

**Host methods called:**
- **`session.openShell {cwd: homedir, command, label, owned:true, tag:'s:<hostId>'|'c:<alias>'}`:** reads `{tabId}`, tolerating null. On failure it shows the banner `não abri o ssh: <msg>` (sticky).
- **`view.open`**, in three forms:
  - `{viewId:'hosts', title:'SSH', rfw:{library, root:'root'}, data}`.
  - `{viewId:'hosts', title:'SSH', blocks}` when rfw is missing.
  - `{viewId:'editor', title, blocks}`.
- **`view.update`:** `{viewId:'hosts', data|blocks}` or `{viewId:'editor', title, blocks}`. Destructured as `{open}` with no guard.
- `view.close {viewId:'editor'}`.
- **`window.form`:** see below.
- **`window.input {title, placeholder, value}`:** used to name a group.
- **`window.pick {title, placeholder?, items:[{value,label,detail?}]}`:** used for the connect picker and for moving a host to a group.
- **`sessions.list`:** reads `id`, `owner`, `tag`, `exited`, `focused`.
- `session.focus {tabId}`, `session.close {tabId}`, `clipboard.write {text}`.
- **`sidebar.update {blocks, badge: String(liveCount)|null}`:** the result is ignored.
- `window.showBanner {text, sticky?}`, `log`.

`window.form` call:
- `title`: `"novo host"` or `` `editar ${name}` ``.
- `fields`:
  - `{id:'name',label:'nome',placeholder,value}`
  - `{id:'host',label:'host',placeholder,value,required:true}`
  - `{id:'user',label:'usuário',value,half:true}`
  - `{id:'port',label:'porta',value,half:true}`
  - `{id:'key',label:'chave privada',…}`
  - `{id:'options',label:'opções do ssh',…}`
  - `{id:'group',type:'select',label:'grupo',value,options:[{value:'',label:'sem grupo'},…,{value:'::novo',label:'novo grupo…'}]}`
  - `{id:'newGroup',label:'nome do novo grupo',required:true,showIf:{field:'group',value:'::novo'}}`
- `buttons`: `{action:'apagar',label:'apagar',style:'danger'}?`, `{action:'duplicar',label:'duplicar'}?`, `{action:'conectar',label:'salvar e conectar'}`, `{action:'salvar',label:'salvar',style:'primary'}`.
- `error`: `''` on the first call, a message on re-calls.
- It loops, re-calling with the returned `values` plus `error`. The result is `{action, values}` or `null`.

**Host messages handled:**
- **`initialize`:** reads `dataDir`, `settings`, `rfw >= 1`, `input >= 1`, and `form >= 1`. Returns `{}`. Without `form` it falls back to the blocks editor window; without `input`, to an inline input.
- **`command.invoke`:** `hosts` opens the window. `conectar` runs the picker unawaited. Unknown ids throw. Returns `null`.
- **`event`:** `sidebar.shown`, `session.closed {tabId}`, `session.exited {tabId}` (after which it expects `exited:true` in `sessions.list`), `session.status {tabId}`.
- **`view.action`:**
  - `hosts` with action `act` resolves to `values.a || values.action`. Any other `hosts`/`editor` action uses the blocks handler. `sidebar` uses the sidebar handler, which reads `values.filtro`.
  - Window verbs: `novo`, `novo-grupo`, `renomear:<g>`, `ask-ok`, `ask-cancelar`, `recolher:<grupo:X|salvos|config>`, `apagar-grupo:<g>`, `mover:<id>`, `novo-no-grupo:<g>`, `grupo:<g>`, `raiz`, `buscar`, `atualizar`, `form-grupo`, `editar:<id>`, `lapis`, `cancelar`, `salvar`, `salvar-conectar`, `duplicar-host:<id>`, `apagar:<id>`.
  - Session-related verbs: `conectar:<id>`, `config:<alias>`, `focar:<tabId>`, `ir:<s:id|c:alias>`, `duplicar:<key>`, `reconectar:<tabId>`, `fechar:<tabId>`, `copiar:<id>`, `copiar-config:<alias>`.
  - The sidebar adds `janela`, `grupos`, `filtrar`.
- `settings.changed {settings}`.
- `shutdown`: the default.

**Blocks:**
- `heading`.
- `input {id:'f<gen>.<field>'|'ask<gen>', label, placeholder, value, submit}`.
- `row {children}`: buttons, and also inputs side by side.
- `select {id,label,value,options,action:'form-grupo'}`.
- `text {style: error|faint}`.
- `button {action,label,style:primary|danger|icon,icon,disabled,tooltip}`.
- `divider`.
- `list {items, empty, flat}`: item fields `title`, `subtitle`, `icon`, `tone`, `badge`, `action`, `actions:[{action,icon,tooltip,tone?}]`, `menu:[{action,label,icon,tone?:'red'}|{type:'divider'}]`. Config-host items also carry a non-standard `connect` field and no `action`.
- `section {text, count (number or "N · M conectados"), collapsed, action}`.

**rfw** (`ui/hosts.rfwtxt`, root `root`):
- Local widgets: `Head`, `Toolbar`, `Action`, `Crumb`, `Section`, `Tile`, `Avatar`.
- Data: `head:{title,sub,newHost}`, `crumb:{group,menu}`, `sections:[{title,count(str),empty(str),items:[{title,sub,icon,tone,status:''|'green'|'red',action,menu,edit}]}]`, `hint`.
- The menu format is `[{label,icon,action,red}|{divider:true}]`.
- Widgets:
  - Maestria: `Glyph` (including `"chevron"`), `IconButton`, `Pressable(menu,onMenu)`, `MenuButton`.
  - Core: `GridView(shrinkWrap, primary, gridDelegate:{type:"maxCrossAxisExtent", maxCrossAxisExtent, mainAxisExtent, mainAxisSpacing, crossAxisSpacing})`, `Container(clipBehavior:"antiAlias", decoration circle/box)`, `Positioned(right,bottom)`, `Wrap`.
- `data` contains no numbers.

**Env and files:**
- `MAESTRIA_PLUGIN_ID` (fallback `'maestria.ssh'`) and `MAESTRIA_PLUGIN_DATA`.
- Files: `hosts.json`, `groups.json`, `collapsed.json`, each written atomically through a `.tmp` file with mode 0600.
- Reads `~/.ssh/config` and its `Include`s.
- The command string it builds is `ssh [extra] -o ConnectTimeout=N [options] [-p port] [-i 'key'] 'user@host'`, or `ssh 'alias'`. The host runs it.

#### 9. argocd (`maestria.argocd`), identical to `~/maestria/argocd-plugin`

Manifest fields:
- `id "maestria.argocd"`, `name "ArgoCD"`, `version "1.0.0"`, `author "Lucas"`, `maestria 1`, `icon "icons/argo.svg"`, `main ["node","main.js"]`.
- `activationEvents`: `onCommand:clusters`, `onCommand:argo`, `onCommand:navegador`, `onCommand:kubeconfig`.
- `permissions ["sessions.create"]`.
- `contributes.sidebar: true`, `contributes.screen: {"home":"clusters"}`.

Commands:
- `clusters`, "argocd: clusters"
- `argo`, "argocd: abrir cluster…", key `meta+alt+a`
- `navegador`, "argocd: abrir cluster no navegador…"
- `kubeconfig`, "argocd: sincronizar kubeconfigs do rancher"

Settings (14):

| id | type | default | group | notes |
|---|---|---|---|---|
| `open` | select | `"maestria"` | — | options `[{value:"maestria",label:"no maestria"},{value:"browser",label:"no navegador"}]` |
| `refresh` | number | 5 | — | |
| `kubeDir` | string | `"~/.kube/clusters"` | `clusters` | |
| `rancherUrl` | string | "" | `clusters` | |
| `rancherUser` | string | "" | `clusters` | |
| `rancherRemember` | boolean | false | `clusters` | |
| `rancherInsecure` | boolean | false | `clusters` | |
| `namespace` | string | `"argocd"` | `argocd` | |
| `username` | string | `"admin"` | `argocd` | |
| `kubectl` | string | `"kubectl"` | `argocd` | |
| `tail` | number | 500 | `logs` | |
| `maxLines` | number | 5000 | `logs` | |
| `tlsDir` | string | "" | `navegador` | |

**Host methods called:**
- **`view.open`:**
  - rfw home: `{viewId:'home', title:'ArgoCD', rfw:{library, root:'home'}, data}`.
  - rfw tree: `{viewId:'a:<cluster>:<ns>:<name>', title:'<name> · <cluster>', rfw:{library, root:'tree'}, data}`. The same library is used with two different roots.
  - When rfw is missing: `blocks` = an OUTDATED heading plus text.
  - Resource view: `{viewId:'r:<cluster>:<ns>:<app>:<uid>', title:'<kind>/<name> · <cluster>', blocks}`.
  - Sync modal: `{viewId:'kubesync', title, modal:true, blocks:[{type:'text',style:'dim',text},{type:'terminal',tabId,expand:true,autofocus:true,empty:'terminal encerrado'}]}`.
- **`view.update`:** `{viewId, data}` (rfw views) or `{viewId, blocks}`. Reads `{open}`.
- **`view.appendLines {viewId, id:'log:<n>', lines}`:** flushed every 150 ms. Lines are strings, `{text,tone:'red'}` or `{text,tone:'faint'}`. Reads `{open}`.
- **`view.close {viewId:'kubesync'}`:** 1.5 s after `session.exited` with `code === 0`.
- **`session.openShell {command, label:'kubeconfig · <cluster>'|'kubeconfig · todos', embedded:true}`:** no cwd, owned or tag. `command` is `env RANCHER_URL=… RANCHER_INSECURE=… KUBECONFIG_DIR=… [RANCHER_USER=…] [RANCHER_REMEMBER='1'] bash '<pluginDir>/bin/rancher-kubeconfig-sync' ['cluster']`. Destructures `{tabId}` with no guard.
- **`window.pick`:** `{title, items:[{value,label,detail}]}`, and `{title, placeholder:'cluster', items:[{value, detail?}]}`. **The cluster items have no `label`**, so the host must show `value`.
- **`window.input {title:'filtrar apps', placeholder:'parte do nome', value}`.**
- `window.openUrl {url}`: `http://127.0.0.1:<port>/` (a local login-cookie page) or `https://<cluster>.argo.localhost…`.
- `window.showBanner {text, sticky?}`.
- `sidebar.update {blocks, badge: connected count|null}`: the result is ignored.
- `log`.

**Host messages handled:**
- **`initialize`:** reads `settings` and `rfw >= 1` only, and returns `{}`. It uses the `MAESTRIA_PLUGIN_DATA` env, not `dataDir`.
- **`command.invoke`:** the 4 ids, unawaited, returning `null`. Unknown ids are silently ignored.
- **`view.action`:**
  - `act` resolves to `values.a || values.action`. One handler serves every viewId, including `sidebar`.
  - View-scoped verbs: `search`, `pick:<c>`, `home`, `pickCluster`, `clearfilter`, `only:<''|outofsync|unhealthy>`, `toggleOld`, `res:<uid|app>`, `tab:<resumo|logs|eventos|manifesto>`, `resreload`, `logpod` (reads `values.pod`), `logcontainer` (reads `values.container`), `logrestart`, `podlogs:<uid>`.
  - Global verbs: `open:<c>`, `here:<c>`, `browserCluster:<c>`, `disconnect:<c>`, `kubesync:<c|''>`, `reload:<c>`, and `app|browser|sync|refresh:<c>|<ns>|<name>` (the args are pipe-separated).
- **`event`:** `sidebar.shown` re-renders. `session.exited {tabId, code}` and `session.closed {tabId}` are matched against the sync terminal. `sidebar.hidden` is not handled.
- **`settings.changed`:** may run a keyring forget, then restarts the timer and re-renders.
- **`shutdown`:** stops logs, port-forwards and proxies, then exits.

**Blocks:**
- `header {title, subtitle, tone, avatar:{text,color}, actions:[{action:'resreload',icon:'refresh',tooltip}]}`.
- `tabs {items:[{label,action,active,icon?,count?}]}`.
- `kv {items:[{key,value(multiline),tone?}]}`.
- `text {style: dim|warning|error|faint}`.
- `section {text,count}`.
- `list {items, empty, flat}`:
  - Pod items: `title`, `subtitle`, `icon`, `tone`, `action`, `actions`.
  - Event items: `title`, `subtitle`, `icon`, `tone`, `meta`.
  - Sidebar items: `title`, `subtitle`, `subtitleTone`, `icon`, `tone`, `avatar:{icon:'server',color}`, `status`, `action`, `actions`, `menu:[{action,label,icon,disabled?}|{type:'divider'}]`.
- `progress {label}`.
- `code {text}`, the YAML manifest with Secrets masked.
- `columns {children: select…}`.
- `select {id:'pod'|'container', label, value, options: plain strings, action}`.
- `row {align:'between', children:[text, button]}`.
- `button {action,icon,tooltip,style:'icon'}`.
- `console {id:'log:<n>', expand:true, max, empty}`.
- `terminal {tabId, expand, autofocus, empty}`.

**rfw** (`ui/argo.rfwtxt`, roots `home` and `tree`):
- Local widgets: `Head`, `Stats`, `Notice`, `Dot`, `Pill`, `Node`, `Tile`, `Crumb`, `ClusterTile`.
- Data:
  - `head:{title,sub,buttons:[{icon,tip,a,tone}]}`, `stats:[{text,tone,a}]`, `notice:{text,tone}`, `crumb:{cluster}`.
  - `clusters:[{name,a,status:'green'|'yellow'|'red'|'none',sub,subTone,buttons:[{icon,tip,a}]}]`.
  - `apps:[{name,a,icon,health,healthText,sync,syncText,op,opTone,rows:[{k,v}],sync_a,refresh_a,browser_a}]`.
  - `graph:{nodes:[{x,y,title,abbr,sub,health,sync,sel,a}], svg, w, h}`.
- Widgets:
  - Maestria: `Glyph(icon:"icons/argo.svg"|name)`, `IconButton`, `Pressable` (no menu), `Svg(svg,width,height)`.
  - Core: nested `SingleChildScrollView(scrollDirection:"horizontal")`, `Positioned(start,top)`, `Spacer`, `Container(margin)`.
- Theme keys: `active`, `accent`, `fg`, `faint`, `dim`, `hover`, `bg`, `border`, `yellow`, `green`, `red`, `purple`.

**Env and files:**
- Env:
  - `MAESTRIA_PLUGIN_DATA`, with fallback `~/.local/state/argo`.
  - `ARGO_RECENT_FILE`.
  - `ARGOCD_PASSWORD_<CLUSTER>` and `ARGOCD_PASSWORD`.
  - `XDG_CONFIG_HOME`.
- Data files: `<data>/recent` and `<data>/tls/argo-localhost.{crt,key}` plus `ca.sha256`.
- Reads kubeconfigs from `kubeDir` (`*.yaml|*.yml|*.kubeconfig`), and reads/writes `~/.config/rancher/username`.

**Binaries:**
- `kubectl`: `get --raw /version`, `get secret argocd-initial-admin-secret`, `port-forward svc/argocd-server -n <ns> <port>:443`, `logs -f --tail=N`.
- `openssl`.
- Keyrings: `secret-tool` on Linux and `security` on macOS, service `maestria-argocd`.
- `bash bin/rancher-kubeconfig-sync`, which runs `curl` and `jq` and reads with `read … </dev/tty`.
- Its own local TLS proxy and a login page that sets the `argocd.token` cookie.
- Calls the ArgoCD HTTPS API. The only write is `POST /api/v1/applications/<n>/sync` without prune.

**Quirk:** graph `x`, `y`, `w` and `h` are deliberately `Math.round(n)+0.5`. They therefore arrive as JSON doubles feeding `SizedBox`, `Svg` and `Positioned`, and the host must keep doubles as doubles.

#### 10. wiboor (`maestria.wiboor`)

Manifest fields:
- `id "maestria.wiboor"`, `name "Wiboor"`, `version "1.9.0"`, `icon "icons/wiboor.svg"`, `author "Matheus Pano"`, `maestria 1`, `main ["node","main.js"]`.
- `activationEvents`: `onCommand:quadro`, `onCommand:abrir`, `onCommand:nova`, `onCommand:branch`, `onSuggestion`.
- `permissions ["sessions.create"]`.
- `contributes.sidebar: true`, `contributes.screen: {"home":"quadro"}`, `contributes.suggestions: {"label":"criar a task no Wiboor"}`.

Commands:
- `quadro`, "wiboor: o quadro", key `meta+alt+k`
- `abrir`, "wiboor: abrir uma tarefa sua…"
- `nova`, "wiboor: nova tarefa…"
- `branch`, "wiboor: a tarefa da branch deste painel"

Settings:
- `autoStart`, boolean, default true
- `detailModal`, boolean, default true
- `finishedDays`, number, default 7
- `refresh`, number, default 60 (minimum 15, 0 turns it off)

**Host methods called:**
- **`window.showBanner {text, sticky?}`.**
- **`window.pick {title, placeholder, items:[{value,label,detail?}]}`:** five uses:
  - finish confirmation (`sim`/`nao`);
  - repo picker (`f.root`, or `aqui:<cwd>`);
  - task picker (task id);
  - person picker (user id);
  - space picker, whose first item has value `''`. **The host must return `""` for that item, as distinct from `null` on cancel.**
- `clipboard.write {text}`, `window.openUrl {url}`.
- **`sessions.list`:** reads `kind` (`claude`), `exited`, `id`, `cwd`, `branch`, `title`, `hibernated`, `statusLabel`, `status`. It maps `status` values `working`, `tool`, `waitingInput`, `waitingAnswer`, `waitingPermission`, `idle`, `ready`, `starting`. It matches sessions to tasks with the regex `#<num>(?!\d)` against `branch` or `title`, and by tabId/cwd.
- **`sessions.focused`:** reads `folder`, `cwd`, `branch`, `title`, or `null`. The `branch` command parses `(?:TASK|BUG)#?(\d{3,})`.
- **`folders.list`:** reads `isRepo`, `root`, `name`, `worktrees[].branch`, `worktrees[].prunable`.
- `session.focus {tabId}`.
- `session.openClaude {cwd, prompt, label}`: reads `{tabId}`.
- **`suggestion.created {suggestionId, ref:'TASK#n'|'BUG#n', intro:'Vamos trabalhar na TASK#n do Wiboor (<link>): <title>', branch:'feature/TASK#n'|'hotfix/BUG#n', dir:'TASK-n'|'BUG-n'}`:** reads `{tabId, cwd}`, or `null`.
- **`view.open`:** `{viewId,title,blocks}`, `{viewId,title,data,rfw:{library,root:'root'}}`, or with `modal:true`.
- **`view.update`:** `{viewId,title?,blocks}` or `{viewId,title,data[,rfw]}`. Checks `r.open === false`.
- `view.close {viewId}`.
- **`sidebar.update`:** `{blocks, badge}` (destructured `{shown}`; a throw is caught) or `{badge}` alone. The badge is `""` or the count of STARTED tasks the user executes.
- `log`.

**Host messages handled:**
- **`initialize`:** reads `dataDir`, `settings`, `rfw` (≥1 for rfw, ≥2 for drag and drop) and `modal >= 1`. Returns `{}`.
- **`command.invoke`:** the 4 ids, fire-and-forget, returning `null`. Unknown ids throw.
- **`suggestion.start {suggestion:{id,title,tldr,prompt}, origin:{isRepo, folder, …}, choice='worktree'|'local'|'here'}`:**
  - Throws `'suggestion.start sem a sugestão ou a origem'` when either is missing.
  - Throws `'a sessão que sugeriu não está num repositório: não há onde criar a worktree'` for a worktree choice outside a repo.
  - Otherwise opens the create form unawaited and returns `null`.
- **`event`:**
  - `session.opened`, `session.closed`, `session.status`, `session.exited`: a 400 ms debounced reload.
  - `sidebar.shown`: push, then a quiet reload.
  - `sidebar.hidden`: handled.
- **`settings.changed`:** reschedules the timer and reloads.
- **`shutdown`:** the default; it also exits on SIGTERM.
- **`view.action`** (the action is resolved as `values.a || values.action || action`):
  - `quadro`:
    - `action==='drop'` reads `values.payload` (a task id or `col:<STATE>`) and `values.to`.
    - `act` goes to the board actions.
  - `nova`: `form:cancelar`, `form:espaco`, `form:criar`.
  - `quadro.personalizar`: `cfg:subir|descer|visivel|recolhida|ordem:<COL>`, `cfg:largura`, `cfg:show:<field>`, `cfg:padrao`, `cfg:fechar`.
  - `tarefa.<id>`: `check:<task>:<item>`, `additem:<task>`, `comentar:<task>`, `recarregar:<id>`, `rascunho:<id>`, `anexo:<task>:<i>:abrir|baixar|mostrar|link`. Anything else goes to the board actions.
  - Board and sidebar verbs:
    - Task: `abrir:<id>`, `start:`, `pause:`, `end:`, `branch:`, `link:`, `web[:id]`, `trabalhar:<id>`.
    - Filters and scope: `rapido:<k|''>`, `aba:<STATE>`, `quem:me|pedi|todos|outra`, `papel:executor|requester`, `espaco:escolher|todos`.
    - Columns and layout: `coluna:<COL>:esquerda|direita|recolher|esconder|ordem:<sort>`, `personalizar`, `secao:<STATE>`.
    - Navigation and refresh: `quadro`, `nova`, `buscar`, `atualizar`, `chave`.
  - Value keys read: any key starting with `chave.`, `ck.<gen>.<item>`, `comentario.<gen>`, `item.<gen>`, `prazo.<gen>` (`YYYY-MM-DD`), `espaco`, `executor.<spaceKey>`, `tipo.<g>`, `prioridade.<g>`.

**Views:**

| viewId | form | content |
|---|---|---|
| `quadro` | rfw, with a blocks fallback | the board |
| `tarefa.<taskId>` | blocks, `modal` when `rfw.modal && detailModal` | one task |
| `nova` | blocks | the create form |
| `quadro.personalizar` | blocks | board configuration |
| sidebar | blocks | the queue |

There are no floats, and no `window.form` or `window.input`.

**Blocks:**
- `header {title, subtitle, tone, avatar:{icon,color}|{text,shape:'circle',color}, status, dim, wrap:true, actions:[{action,icon,tooltip,disabled?}|{icon,tooltip,menu}]}`.
- `section {text, count(number|string), collapsed, action}`.
- `text {text, style: faint|dim|error|warning|success}`.
- `markdown {text}`: includes `[![name](url)](url)` image links.
- `input {id,label,placeholder,value,multiline,submit,icon:'add'}`.
- `checkbox {id,label,value,action,disabled}`.
- `select {id,label,value,placeholder,action,options}`: options are strings or `{value,label}`.
- `calendar {id, mode:'single', min:'today'}`.
- `button {action,label,icon,style:'primary',tooltip,disabled}`.
- `row {children, align:'between'|'end'}`, `columns {children}`.
- `card {children, padding:6|10}`.
- `tabs {items:[{label,action,active,count}]}`.
- `list {flat, empty, alwaysActions, items}`: item fields `title`, `subtitle`, `subtitleTone`, `icon`, `tone`, `avatar`, `status`, `dim`, `meta`, `metaTone`, `action`, `actions:[{action,icon,tooltip}]`, `menu:[{action,label,icon,disabled}|{type:'divider'}]`.
- `progress {value?, label, detail, tone}`: `value` can be the int `1`.
- `divider`.

**rfw** (`ui/kanban.rfwtxt`, root `root`):
- The library is sent only on the first push after open or reopen; afterwards only `data`.
- Widgets:
  - Maestria: `Pressable(radius,color,hoverColor,padding,onTap,menu,onMenu)`, `MenuButton(icon:'filter',…)`, `IconButton`, `Glyph` (`"icons/comment.svg"`, `"icons/attach.svg"`, `"task"`, …), `Tooltip`, `Draggable(payload,targets,disabled,width,child)`, `DropTarget(id,hint,radius,onDrop)` nested two deep (column slot `col:<ID>` and state id).
  - Core: `Rotation`, `FractionallySizedBox(widthFactor)`, `IntrinsicHeight`, `Wrap`, `Spacer`, `Stack`, `Positioned`, `ListView`, `SingleChildScrollView`, `Align`, `Center`.
- Events: `event "act" {a}`, `event "act" {}` (the menu adds `action`), and `event "drop" {}` (the host adds `payload` and `to`).
- Data:
  - `drag`, `head:{title,sub,busy,menu,scopes:[{label,icon,action,active}]}`, `stats:[{text,tone,action,active}]`, `statsSkeleton`, `notice:{text,tone,retry}`, `layout:{width:'narrow'|'normal'|'wide'}`.
  - `cols:[{id,slot,slots,label,tone,icon,hint,empty,invite,count,skeleton,collapsed,toggle,sortLabel,menu,cards,peek,more}]`.
  - Card fields: `{id,action,kind,num,title,sub,due,dueTone,late,checks,comments,files,progress,prio,prioTone,running,claude:{text,tone,action},targets,fixed,who:{initials,name,tone},bar:{show,value(clamped 0.02–0.999),label,done},acts:[{icon,tooltip,action,primary}],menu}`.
- Theme keys: `fg`, `faint`, `dim`, `accent`, `onAccent`, `bg`, `canvas`, `sidebar`, `hover`, `active`, `border`, `green`, `yellow`, `red`, `purple`, `blue`, `cyan`, `magenta`, `mono` (a string).

**Env, files, HTTP and binaries:**
- Env: `MAESTRIA_PLUGIN_DATA` (overridden by `dataDir`), `WIBOOR_API_KEY`, `WIBOOR_CONFIG`, `WIBOOR_BASE_URL`.
- Files:
  - `<data>/state.json` `{filter,formSpace,links,tab,layouts}`.
  - `<data>/anexos/<TASK-n>/<i>-<name>` (images for the claude prompt).
  - `~/.config/wiboor/config.json` `{apiKey,authHeader,authScheme,baseUrl}`, written chmod 600.
  - `~/Downloads/<name>`.
  - A temp dir `maestria-wiboor-*/prompt.md`.
- HTTP against `https://api.azzimuti.com.br/v1` with Bearer JWT auth:
  - `GET /departments`, `GET /tasks?…`, `GET /tasks/:id`.
  - `POST /tasks/:id/start|pause|end`, `POST /tasks/:id/comments`, `POST /tasks/:id/checklist`, `POST …/checklist/:item/check|uncheck`, `POST /tasks`.
- `git`: worktrees in `<repo>/.claude/worktrees/TASK-n`, and `config branch.<b>.description`.
- `/bin/zsh -lc "claude -p --output-format text < file"` (3 min).
- `open` or `xdg-open`.

---

---

## 6. Test cases (language-neutral given/when/then)

Derived from `test/plugins_test.dart` (70), `plugin_modal_test` (4), `plugin_screen_test` (14), `floats_test` (3), `suggestion_plugins_test` (12), plus `quick_input_test` (8), `rail_badge_test` (2) and single tests in `reveal_test`, `shortcuts_test`, `theme_test`. "The API" = the plugin→host dispatcher (§2.4); every failure is a JSON-RPC error.

### 6.0 Helpers, fixtures and fakes

**`writePlugin(parent, name, manifest, {files})`**, in `plugins_test`
- Creates `<parent>/<name>/maestria-plugin.json` (the constant `mxManifestName`) and any extra files, making subfolders as needed.
- `plugin_screen_test` has its own version, `writePlugin(parent, name, manifest, [script])`. It writes `plugin.sh` and runs `chmod +x` on it.

**`aurora`**
- Theme JSON loaded from `examples/plugins/tema-e-atalhos/themes/aurora.json`.
- Values relied on: `id: "exemplo-aurora"`, `label: "Aurora"`, `accent: #5EE3C1`, `ansi.brightBlack: #5F7385`, and no `brightRed`.

**`until(ok)`**
- Polls 100 times at 30 ms intervals (about 3 s), then asserts. The other files repeat the same loop inline.

**`shPlugin`** (the shell fixture in `plugins_test`), verbatim:
```sh
#!/bin/sh
idof() { echo "$1" | sed 's/.*"id":\([0-9]*\).*/\1/'; }
while IFS= read -r line; do
  case "$line" in
    *'"method":"initialize"'*)
      echo "{\"jsonrpc\":\"2.0\",\"id\":$(idof "$line"),\"result\":{}}" ;;
    *'"method":"command.invoke"'*)
      echo '{"jsonrpc":"2.0","id":"p1","method":"window.showBanner","params":{"text":"oi"}}'
      echo 'isto não é json'
      echo "{\"jsonrpc\":\"2.0\",\"id\":$(idof "$line"),\"result\":\"feito\"}" ;;
    *'"method":"event"'*)
      echo '{"jsonrpc":"2.0","method":"log","params":{"message":"evento"}}' ;;
    *'"method":"shutdown"'*)
      exit 0 ;;
  esac
done
```
- **Wire-format requirement:** the host must serialize compact JSON with no spaces, e.g. `"method":"initialize"`, because the fixtures match on substrings.
- **Request-id requirement:** the request `id` must be numeric, and it must be the only numeric value that follows `"id":` on the line.
- **Reverse requests:** the plugin sends requests back to the host with the string id `"p1"`.

**Fixture `morre.sh`:** `#!/bin/sh\nexit 3\n`

**Fixture in `plugin_screen_test`** (the board):
- Same loop as above.
- On `command.invoke` it sends `{"jsonrpc":"2.0","id":"p1","method":"view.open","params":{"viewId":"quadro","title":"Quadro","blocks":[]}}`, then replies `{"jsonrpc":"2.0","id":<id>,"result":null}`.
- `initialize` and `shutdown` behave as in `shPlugin`.

**Fixture `_taker`** (`suggestion_plugins_test`):
- `idof` uses `sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p'`. Only the numeric id counts, because the suggestion's own `id` is a string.
- On `suggestion.start` it replies `{"jsonrpc":"2.0","id":<id>,"result":null}`, then sends `{"jsonrpc":"2.0","id":"p1","method":"suggestion.created","params":{"suggestionId":"task_00000001","ref":"TASK#1","intro":"Vamos trabalhar na TASK#1","branch":"feature/TASK#1","dir":"TASK-1"}}`.
- `initialize` and `shutdown` behave as in `shPlugin`.

**Fakes and stubs:**
- **`Plugins.onCall`:** stub for plugin→host requests, `(plugin, method, params) async`. Tests either record `"$method ${jsonEncode(params)}"` and return null, or return `{'tabId':'nova'}`. The end-to-end tests wire it to `PluginApi(store).handle`.
- **`_ActionsStore`:** an `AppStore` that overrides `pluginViewAction(tab, action, values)` to record `actions` and `values` instead of sending `view.action`.
- **`_ShellStore`:** an `AppStore` that overrides `openShell(...)`. It opens no pty, records the `place` flag in `placed`, and creates `MxTab(id: 'tab${n+1}')`.
- **Window stubs on the store:** `store.quickPick`, `store.quickInput` and `store.quickForm` are injectable UI callbacks. When null, the API errors because there is no window.
- **Fake plugins:** `MxPlugin(dir:'/p', manifest: const PluginManifest(...))` is an in-memory plugin with no process.
- **`desk(tmp)`** (modal and screen tests): store with folder `/repo/app`, two shell tabs `esquerda` and `direita`, `PaneSplit(row, [left,right], [0.3,0.7])`, focus on `direita`. In the modal test it also installs the Wiboor plugin.
- **`storeWithOne()`** (suggestions): folder `/repo` (`isRepo=true`, name `meu-repo`), claude tabs `a` (label "implementa") and `b`, pane = `b`, focus = `b`.
- **Terminal output capture:** `tab.term.terminal.onOutput` records what would be written to the pty.
- **`tab.view.debugValues`:** the pane's current form values map.

---

### 6.x test/plugins_test.dart

Every test gets a fresh temp dir. Teardown clears `MxThemes.extra` (themes registered by plugins).

#### group "o manifesto"

| ID | Given | When | Then |
|---|---|---|---|
| plugins:manifesto:minimo | `{"id":"meu.plugin","version":"0.1.0"}` | parse | `name == "meu.plugin"` (falls back to id); `hasProcess` false; `commands` empty |
| plugins:manifesto:recusa | base `{id:"x",version:"1"}` plus each override | parse | Throws a format error whose message contains: `{"id":"Com Espaço"}` → `"id"` · `{"permissions":["root"]}` → `permissão desconhecida` · `{"maestria":99}` → `versão 99` · `{"main":3}` → `"main"` · a command `{id:a,title:A}` with no run/send/main → `não tem "main"` · a command with both `run:"ls"` and `send:"oi"` → `escolha um` · a command with `run:"ls", in:"nuvem"` → `"in"`. Full source texts: `"id" só pode ter minúsculas, dígitos, ponto, hífen e sublinhado: "<id>"`, `permissão desconhecida: "<p>" — as que existem são …`, `o plugin pede a versão 99 da API e esta MaestrIA só conhece até a 1`, `"main" é uma lista: ["node", "main.js"]`, `o comando "a" não tem "run" nem "send", e o plugin não tem "main" pra atendê-lo`, `o comando "a" tem "run" e "send" — escolha um`, `o comando "a" tem "in": "nuvem" — vale "terminal" ou "background"` |
| plugins:manifesto:tres-jeitos | `main:"bin/plugin"` (a string); commands a: none; b: `run:ls, in:background`; c: `send:revise`; d: `run:htop` | parse | `main == ["bin/plugin"]` (a string becomes a 1-element list). Targets in order: plugin, background, session, terminal. `commands[0].fullId == "x/a"` |
| plugins:manifesto:tecla-invalida | commands with `key` set to `meta+q`, `meta+alt+g`, `banana+z+` | parse | `meta+q` → no key (reserved); `meta+alt+g` → chord G with meta+alt; `banana+z+` → no key. `warnings` has length 2; `warnings[0]` contains `⌘Q`. **Platform caveat:** in the source, `forHost` maps meta to Ctrl+Shift when `Platform.isLinux`, so this expectation only holds on macOS. Warning texts: `o comando "<cid>" pede a tecla "<k>", que não dá pra ler — ficou sem` / `o comando "<cid>" pede <label>: <why> — ficou sem` |
| plugins:manifesto:activation | (a) `main:["node","m.js"]` with no activationEvents; (b) the same plus `activationEvents:["onHook:Stop"]`; (c) no main | `activatesOn(...)` | (a) `onStartup` true, `onHook:Stop` false. (b) `onStartup` false, `onHook:Stop` true. (c) `onStartup` false: there is nothing to start |
| plugins:manifesto:exemplos | `examples/plugins/tema-e-atalhos` and `exemplo-completo` | read from disk | `warnings` empty for both |

#### group "o tema de plugin"

| ID | Given | When | Then |
|---|---|---|---|
| plugins:tema:le-json | aurora | `MxPalette.fromJson` | id `exemplo-aurora`; accent `#5EE3C1`; `ansi.brightBlack` `#5F7385`; `ansi.brightRed == ansi.red` (missing bright falls back to the normal colour) |
| plugins:tema:campo-faltou | aurora without `bgHover`; aurora with `ansi.cyan:"azul"` | fromJson | Format error message contains `bgHover`; then a format error containing `ansi.cyan` |
| plugins:tema:byId | aurora added to the extra themes | `byId` | `byId('exemplo-aurora').label == 'Aurora'`; unknown id → the default theme (`maestria`) |

#### group "a pasta de plugins"

| ID | Given | When | Then |
|---|---|---|---|
| plugins:pasta:bons-ruins-dup | Dirs: `a-tema` (`id a.tema`, version, `contributes.themes:["t.json"]` = aurora); `b-quebrado` (`{"id":"b"}`, no version); `c-repetido` (`id a.tema`, version 2); and a `.staging` dir | `Plugins(root).scan()` | States in directory-name order: idle, invalid, invalid (`.staging` is skipped). `[1].problem` contains `"version"` (source: `"version" é obrigatório`). `[2].problem` contains `já usa o id` (source: `outro plugin já usa o id "a.tema"`); the first plugin by dir order wins. Extra themes = `["exemplo-aurora"]`. `byId('a.tema').palettes` has length 1 |
| plugins:pasta:desligar | plugin `p` with a theme and command `{id:ls,title:listar,run:ls}` | `setEnabled(p,false)` | `commands` empty; extra themes empty; `toJson() == {"disabled":["p"]}`. A new `Plugins` that `load(json)`s then scans shows `p` as `disabled` |
| plugins:pasta:tema-embutido | theme JSON = aurora with `id:"nord"` (a built-in id) | scan | Not registered; `p.log` has exactly one line, which contains `embutido` (source: `"nord" é o id de um tema embutido`). The plugin stays loaded |

#### group "instalar"

| ID | Given | When | Then |
|---|---|---|---|
| plugins:instalar:pasta | source folder `fonte` with `{id:meu.plugin,version:1.0.0}`; root `<tmp>/plugins` | `stage(src)` → `commit` → edit source to 1.1.0 → `stage` → `commit` → `uninstall` | First stage: `manifest.id` is `meu.plugin`, `replacing` null, nothing in `root/meu.plugin` yet. After commit: `plugin.dir == root/meu.plugin`, manifest file present, staging dir deleted. Second stage: `replacing == "1.0.0"` (the old version); after commit there is a single plugin at version 1.1.0. Uninstall: list empty, dir gone, source untouched |
| plugins:instalar:pacote | `pacote/maestria-plugin-x-main/<manifest>` plus an empty `pacote/__MACOSX` | `stage(pacote)` then `discard` | `staged.dir` ends with `maestria-plugin-x-main` (it descends into the single real subfolder, ignoring `__MACOSX`). Discard deletes the staging dir |
| plugins:instalar:recusa | an empty folder; a missing path | `stage` | Install error whose message contains `maestria-plugin.json`; a missing path also gives an install error. `<root>/.staging` ends up empty |
| plugins:instalar:link-dev | dev folder `{id:dev.plugin,version:0.0.1}` | `link(dev)` | `plugin.linked` true; `root/dev.plugin` is a symlink. Linking again throws an install error. `uninstall` removes the symlink and keeps the source manifest |

#### group "o processo do plugin"

**Setup:**
- Plugin `sh` with `main:["./plugin.sh"]` (`shPlugin`, chmod +x).
- `activationEvents:["onCommand:oi","onSession"]`, command `{id:oi,title:"dizer oi"}`.
- `onCall` records calls. Teardown runs `stopAll`.

| ID | Given | When | Then |
|---|---|---|---|
| plugins:processo:nao-sobe | setup | `startup()` | State stays `idle` (no `onStartup`) |
| plugins:processo:comando | setup | `invoke(command, {"cwd":"/repo"})` | Result `"feito"`. `calls == ['window.showBanner {"text":"oi"}']`: the plugin's reverse request is routed to `onCall` with params as sent. State `running`. Log has a line containing `stdout: isto não é json` (bad stdout line is logged and the conversation survives). Implies initialize happens before command.invoke and the reverse request is answered |
| plugins:processo:evento-declarado | setup | `emit('session.opened', {tabId:'tab1'}, activations:['onSession'])` | Within about 3 s some log line ends with `evento`: the process was started, received an `event` message and its `log` notification was recorded |
| plugins:processo:evento-nao-declarado | setup | `emit('hook', {name:Stop}, activations:['onHook:Stop'])`; then the same with `activations:['onSession'], needs: hooks` | After 100 ms state is still `idle`. Undeclared activation doesn't start it; a hook event needing the `hooks` permission isn't delivered or activated without it |
| plugins:processo:reiniciar | invoked once (running) | `restart(plugin)` then `stopAll()` | After restart `running`; after stopAll `idle` |

#### Top-level tests (no group)

| ID | Given | When | Then |
|---|---|---|---|
| plugins::main-inexistente | `main:["programa-que-nao-existe-em-lugar-nenhum"]`, no activationEvents | `startup()` | State becomes `crashed`; `crash` contains `não achei`. Full: `não achei "programa-que-nao-existe-em-lugar-nenhum" — nem na pasta do plugin, nem no PATH` |
| plugins::morre-na-largada | `main:["./morre.sh"]` (exit 3), `activationEvents:["onSession"]` | emit with `onSession`, wait for `crashed`, emit again | After the second emit plus 100 ms, the log length is unchanged: no restart attempt. A crashed plugin is not revived by events |

#### group "a tecla do plugin"

Fixed chord: E with meta+alt. The synthetic key event has physical key E, logical key `0xB4` and character `´`: macOS dead-key ⌥E.

| ID | Given | When | Then |
|---|---|---|---|
| plugins:tecla:dead-key | meta and alt held down | the chord's normal `accepts(deadE)`; `Plugins.chordHits(chord, deadE)` | Normal accept is false (logical key mismatch). `chordHits` is true: it matches by physical key position when modifiers match |
| plugins:tecla:sem-alt | only meta held | `chordHits` | false |

#### group "as peças da central"

| ID | Given | When | Then |
|---|---|---|---|
| plugins:central:sidebar-teto | five commands `{run:ls, icon:'bug', sidebar:true}` plus one without sidebar | parse; write a plugin and scan | `commands.first.icon == 'bug'`; `last.sidebar` false. `sidebarCommands.length == maxSidebarCommands` (3 in source): capped |
| plugins:central:console-persiste | view with `console{id:log, lines:['um',{text:'dois',tone:'red'}], max:3}` | `append(['três','quatro'])`; `setBlocks([button, console{id:log}])`; `setBlocks([])` | Lines become `['um','dois']` with the last tone red. After append: `['dois','três','quatro']` (max drops the oldest). Redefining the console without lines keeps the same console object and its 3 lines. Removing it from the blocks deletes it (`consoles` empty) |
| plugins:central:widgets | plugin view `central`, title `flutter`; blocks: `row[select{id:cfg,options:['Debug'],value:'Debug',disabled:true}, button{style:'icon',icon:'reload',tooltip:'hot reload',action:'reload',tone:'yellow'}]` and `console{id:log,expand:true,lines:['linha um']}`; pane 800×600 | render; `store.pluginConsole(plugin,'central','log').append(['linha dois'])` | No error. Tooltip `hot reload` present; the `reload` icon maps to the bolt glyph. `linha um` shown; the select is non-interactive (disabled). Console height > 400 (expand fills the remainder). Appended `linha dois` appears without a store notify |

#### group "seletor, ícone e overflow"

| ID | Given | When | Then |
|---|---|---|---|
| plugins:seletor:window.pick | store with no `quickPick` | API `window.pick {items:[{value:'a'}]}` | Throws: no window. With a stub returning the last item's value, `window.pick {title:'qual projeto?', items:[{value:'/a',label:'a',detail:'~/a'},{value:'/b'},{label:'sem valor'}]}` returns `'/b'`. The stub received `['qual projeto?', 'a\|~/a', '/b\|null']`: label defaults to value, and an item without `value` is dropped |
| plugins:seletor:quick-pick-ui | quick pick titled `projeto` with items app-a (`/x/app-a`), app-b, outro (`/y/outro`) | type `app`, press ↓, press enter | `outro` is filtered out; the result is `/x/app-b` (arrow moves the selection, enter picks) |
| plugins:seletor:svg-icon | plugin dir with `icons/logo.svg` | `PluginGlyph.svgOf(...)`; render glyphs `icons/logo.svg`, `bug`, `nao-existe` | `svgOf('icons/logo.svg')` found; `'../p/icons/logo.svg'` → null (no escaping the dir); a missing file → null. Rendering: one SVG, the `bug` icon (bug_report glyph), and an unknown name falls back to a generic extension/puzzle glyph |
| plugins:seletor:overflow | row: select `cfg` (width 280), select `dev` (width 220) whose option label is `sdk gphone64 arm64 · android-arm64 · um nome bem comprido mesmo`, two icon buttons (`continue`, `refresh`); pane 720×300 | render | No layout exception (long labels truncate) |

#### group "o lugar das janelas na lateral"

| ID | Given | When | Then |
|---|---|---|---|
| plugins:lateral:janela-na-aba | plugin `maestria.flutter` (name Flutter, icon bug) with commands `reload` "flutter: hot reload" and `outro` "git: outro prefixo"; folder `/repo/maestria_v2` with a shell tab labelled `Plugin` in focus | `openPluginView(flutter, 'central-x', title:'flutter · learning-app')`; render the sidebar; `showSidebarView(flutter.id)` | The view's folder is the loose folder (it doesn't inherit the focused pane's folder). It is not in that folder's tabs, nor in the loose tabs list. `viewsOf(flutter)` = [view]. Sessions sidebar shows `Plugin`, not the view title. After switching to the plugin tab: `Plugin` is gone (the tab replaces the whole sidebar); `flutter · learning-app` is shown. Command titles: `hot reload` (the prefix `flutter: `, the plugin name lowercased, is stripped); `git: outro prefixo` is kept |
| plugins:lateral:faixa-click | `ssh` (name SSH, icon terminal, command `hosts` sidebar:true); `relatorio` (no icon, command `dia` "relatório" sidebar:true) | `railPlugins`, `footerCommands`; tap the first glyph twice; `showSidebarView(null)`; `showSidebarView('ssh')` then disable ssh | `railPlugins == ['ssh']` (needs an icon to get a rail spot); `footerCommands == ['relatorio/dia']` (no icon → footer button); `shownPlugin` null. First tap: shownPlugin `ssh`, sidebar not hidden. Second tap on the already-shown tab: sidebar hidden. `showSidebarView(null)`: unhidden, shownPlugin null. Disabling the shown plugin → shownPlugin null |
| plugins:lateral:faixa-reordenar | plugins a, b, c, each with icon terminal and a command | drag the first rail icon down 1.6× the rail width over 400 ms; tap; disable a; `moveRailPlugin('c',0)`; enable a | Order `['a','b','c']` → after drag `['b','a','c']`, and `railOrder` is persisted as `['b','a','c']`. A tap selects `b` and does not reorder. With `a` disabled and `c` moved to 0: `['c','b']`; re-enabling a: `['c','b','a']` (a keeps its saved place relative to the others) |
| plugins:lateral:faixa-vazia | no plugins | render the rail; tap `+` | Rail empty, the sessions icon (space_dashboard) is still present; `+` opens the install dialog (text `Instalar plugin`) |

#### group "a aba que o plugin desenha"

| ID | Given | When | Then |
|---|---|---|---|
| plugins:aba:manifest-sidebar | `main:['node','main.js'], contributes.sidebar:true` | parse | `sidebar` true; no warnings. `sidebar:true` without main → format error (`"contributes.sidebar" precisa de "main" pra desenhar a aba`); `sidebar:'sim'` → format error (`"contributes.sidebar" é true ou false`) |
| plugins:aba:sidebar.update | `git` (main, sidebar:true) and `q` (main, no sidebar); sidebar view = git; sidebar rendered at 320×700 | `sidebar.update` from q; `view.open {viewId:'sidebar'}` from git; then git `sidebar.update {badge:3, blocks:[section{text:'mudanças',count:1}, list{flat:true, items:[{title:'lib/main.dart',action:'abrir'}]}]}`; then `{badge:null}` | `railPlugins == [git]` (sidebar plugin gets an icon even with no commands). q → error (not declared); `viewId 'sidebar'` → error (reserved). Before the first update the sidebar shows `Abrindo Git…` (source: `Abrindo ${name}…`, or `${name} caiu: ${crash}` when crashed). Update returns `{"shown": false}` (not shown via a rail click). `sidebarBadges['git'] == '3'` (the number is stringified). Renders `mudanças` and `lib/main.dart`. With `{badge:null}` badges are empty and the blocks are kept |

#### group "o terminal que é do plugin"

| ID | Given | When | Then |
|---|---|---|---|
| plugins:terminal-dono:owned | plugins ssh and q (both with main); loose shell `t1` with owner `ssh`, ownerTag `s:h1`; loose shell `t2` with no owner | loose tabs list; `ownedBy`; rail; `sessions.list` from ssh and from q; `session.close` from ssh on t2; `session.close` from q on t1; remove ssh from the plugin list | Loose = `[t2]` only; `ownedBy(ssh) == [t1]`; `railPlugins == [ssh]` (owning a terminal gives a rail spot). ssh's list: first item has `owner == 'ssh'`, `tag == 's:h1'`. q's list: no `tag` key (tag is private to the owner). Closing a terminal you don't own → error (both cases). Removing ssh returns its terminal to the loose list: `[t1, t2]` |

#### group "as configurações de plugin"

Manifest settings:
- `colors.app`: color, default `''`, title `log do app`, group `cores`
- `colors.error`: color, default `red`, title `erro`, group `cores`
- `max`: number, default 5000
- `modo`: select, options `['a','b']`
- `ligado`: boolean

| ID | Given | When | Then |
|---|---|---|---|
| plugins:config:tipos | manifest | parse; `resolve` | Types are color, color, number, select, boolean. For `modo`: `resolve(null)='a'` (no default → first option), `resolve('b')='b'`, `resolve('z')='a'`. `max.resolve('muito')=5000` (wrong type → default). `ligado.resolve(null)=false`. A select without options → format error containing `options` (`a configuração "s" é select e não tem "options"`) |
| plugins:config:persistencia | scanned plugin | `settingsOf`, `setSetting`, `toJson`, reload | `colors.error == 'red'`; `toJson()` is `{}` (empty, no `disabled` key). After setting `colors.app = #4FC1FF`: `toJson == {"settings":{"cfg":{"colors.app":"#4FC1FF"}}}`; a reload restores it. Setting null resets it to the default `''` and `toJson` is empty again (back to default = removed from config) |
| plugins:config:cor | — | `pluginColor` | `#4FC1FF` → that colour; `''` → theme fg; `red` → theme red; `azul` → null; `#12` → null |
| plugins:config:dialogo | settings dialog | open; tap the preset swatch tooltip `#4FC1FF`; type `#zz` into the field showing `red`; type `#FF0000`; tap `Voltar ao padrão` | Shows group `cores` and title `log do app`. Preset sets `colors.app=#4FC1FF` and the field shows it. Invalid hex is not saved (stays `red`); a valid one saves `#FF0000`. Reset → `colors.app == ''` |

#### group "o calendário"

Calendar widget fed with range/from/to/max; it records `(from, to)` changes.

| ID | Given | When | Then |
|---|---|---|---|
| plugins:calendario:periodo | range, to = max = 2026-09-23 | tap 19, then 15 | Header `Setembro de 2026`. Emits `[(09-19,09-19), (09-15,09-19)]`: the first click gives a 1-day range, the second closes the range in either order |
| plugins:calendario:max | single mode, to = max = 2026-09-23 | tap 28; next-month arrow; prev-month arrow; tap 25 | 28 ignored (after max). Next month blocked (still `Setembro de 2026`). Prev → `Agosto de 2026`. Tap 25 → `(2026-08-25, 2026-08-25)` |
| plugins:calendario:painel | view: `calendar{id:'periodo', mode:'range', value:{from:'2026-09-22',to:'2026-09-22'}, max:'2026-09-23'}` plus button `ir` | tap 10, tap 18, then the plugin re-sends the same blocks with the old value | Form values `periodo == {"from":"2026-09-10","to":"2026-09-18"}`: the user's choice survives the redraw because the plugin's value didn't change |

#### group "o seletor"

`device(scanning)`:
- select `device`, width 260, `loading: scanning`.
- While scanning, its only option is `{value, label:'procurando aparelhos…'}`.
- Otherwise the options are `[{value:'',label:'aparelho automático'},{value:'emu',label:'sdk gphone64 · android-arm64'}]`.

| ID | Given | When | Then |
|---|---|---|---|
| plugins:seletor:loading | `row[device(scanning:true)]` | render; `updatePluginView` with scanning false; then with value `''` | Shows `procurando aparelhos…` and a spinner. After the update: no spinner, shows `sdk gphone64 · android-arm64`. With value `''` it shows `aparelho automático` (empty-string option is valid) |

#### group "o comando declarativo"

| ID | Given | When | Then |
|---|---|---|---|
| plugins:declarativo:expand | values `cwd="/repo/it's"`, `title='x $(rm -rf ~)'` | `AppStore.expandCommand(template, values, quote)` | Quoted: `cd ${cwd} && echo ${title} ${nada}` → `cd '/repo/it'\''s' && echo 'x $(rm -rf ~)' ${nada}` (single-quote escaping; an unknown var is left as-is). Unquoted: `revise ${cwd}` → `revise /repo/it's` |

#### group "a janela do plugin"

Fake plugin: `id p`, name `Plugin P`.

| ID | Given | When | Then |
|---|---|---|---|
| plugins:janela:uma-por-id | — | open view `v` (title `um`), open `v` again (title `dois`, blocks `[text 'oi']`); update `outra`; close `v` | Same tab object; 1 tab; title `dois`; subtitle `Plugin · Plugin P`; tab is passive; `view.revision == 1`. Updating an unknown view returns false. Close → no tabs |
| plugins:janela:todos-blocos | a plugin tab (plugin not installed) with blocks: heading `Título`; text (style dim); markdown `**negrito**`; code; divider; kv `{key:turnos,value:'3',tone:green}`; progress 0.4 `metade`; list item `{title,subtitle,icon:check,badge:novo,action:a}`; input `nome` (label nome, value `ana`); checkbox `ok`; select `cor` (options `['azul',{value:'v',label:'verde'}]`, value `v`); row `[button primary 'ir', input 'lado' placeholder 'ao lado']`; `{type:'carrossel'}` | render; type `bia` into the `ana` field, then re-set the same blocks and notify | No exceptions. Shows `Título`, `ana`, `verde`, `Bloco desconhecido: carrossel` (source: `Bloco desconhecido: ${type ?? '(sem "type")'}`), and the banner `O plugin não está mais instalado` (or the plugin's crash text if it crashed). After the redraw `bia` is kept |
| plugins:janela:acoes-hover | list item `main.dart`, badge M, action diff; `actions`: descartar (icon undo, tooltip descartar), preparar (icon add, tooltip preparar), and `{action:'sem-icone'}` | render; hover; tap the add icon; tap the row | Action buttons are hidden until hover. On hover: undo and add icons, tooltips descartar and preparar; 3 icon buttons in total (two actions plus the header close). An action without an icon is not rendered. Actions recorded: `['preparar','diff']` |
| plugins:janela:menu-direito | `list flat`, item `LLM`, action conectar; `menu: [{action:editar,label:'editar…',icon:edit},{type:divider},{action:apagar,label:apagar,tone:red}]`; bare pane | right-click `LLM`; pick `editar…` | Menu hidden initially; right-click shows `editar…` and `apagar`. Actions `['editar']` (right-click doesn't fire the row action) |
| plugins:janela:follow-false | `console{id:d, follow:false, expand:true, lines:['linha 0']}` | append `linha 1`..`linha 299` | `linha 0` is still visible, `linha 299` is not (no auto-scroll) |

#### group "a interface em widgets (rfw)"

The shared rfw library (verbatim):
```
import core.widgets;
import maestria;
widget root = Column(children: [
  Text(text: data.head.title, style: { color: data.theme.accent }),
  ...for row in data.rows:
    Pressable(onTap: event "act" { a: row.action }, menu: row.menu, onMenu: event "act" {}, child: Text(text: row.title)),
  IconButton(icon: "stop", tooltip: "parar", onPressed: event "act" { a: "stop:1" }),
]);
```
The tab is a plugin view with id `sidebar`, title `docker`, rendered bare at 400×600.

| ID | Given | When | Then |
|---|---|---|---|
| plugins:rfw:monta-e-eventos | data: `head.title 'Containers'`; one row `{title:'api', action:'abrir:1', menu:[{label:'Parar',icon:'stop',action:'parar:1'},{divider:true},{label:'Remover',action:'remover:1',red:true}]}` | render; tap `api`; tap `parar`; right-click `api` → `Parar`; then `setRfw(data:{rows:[{title:'worker',action:'abrir:2',menu:[]}]})` | Shows `Containers` in the theme accent colour (`data.theme` is injected by the host). Actions `['act','act']`, then a third from the menu (via `onMenu`). New data: `api` gone, `worker` shown, `Containers` still shown (data merges per top-level key; the library isn't re-sent) |
| plugins:rfw:drag-drop | board library: a Row of DropTargets (`id:col.id`, `hint:col.hint`, `radius:8.0`, `onDrop: event "drop" {}`), each holding Text(col.id) and Draggables (`payload:card.id`, `targets:card.targets`, `width:120.0`). Columns: todo (hint `solte pra voltar`, card `{id:t1,title:'cartão',targets:['doing']}`), doing (`solte pra iniciar`), done (`solte pra finalizar`) | drag the card to `done` and release; drag to `doing` and release | Over done the hint is not shown, and releasing records no action. Over doing: hint `solte pra iniciar` shown; releasing → actions `['drop']`, values `{"payload":"t1","to":"doing"}` |
| plugins:rfw:submenu | row `api` menu `[{label:'Cor',icon:'dot',action:'',children:[{label:'Automática',action:'cor:n:api:',color:0},{label:'Laranja',action:'cor:n:api:orange',color:0xFFF2762E}]}]` | right-click → `Cor` → `Laranja` | Submenu opens; a colour dot `#F2762E` is rendered; actions `['act']` |
| plugins:rfw:alvo-aninhado | outer DropTarget (`id:col.slot`) wraps inner DropTarget (`id:col.id`); column header Draggable (`payload:col.slot`, `targets:col.slots`); card Draggables. Cols: todo (slot `col:todo`, slots `['col:doing']`, title `A fazer`, card t1→`['doing']`), doing (slot `col:doing`, slots `['col:todo']`, title `Fazendo`) | drag `A fazer` onto `Fazendo` (+60 px); drag `cartão` onto `Fazendo` (+60 px) | Values `[{"payload":"col:todo","to":"col:doing"}, {"payload":"t1","to":"doing"}]`: the innermost target that accepts the payload wins; otherwise it falls through to the outer one |
| plugins:rfw:alvo-tamanho | horizontal ListView of nested DropTargets around SizedBox(width 200) | render; mouse moves todo → doing | No exception; text width is 200 (target sizes to its child under unbounded width) |
| plugins:rfw:erro-parse | library `widget root = Column(children: [` | render | Text containing `a interface do plugin não leu` (source: `a interface do plugin não leu:\n$e`) |
| plugins:rfw:view.open-rfw | docker plugin | API `view.open {viewId:'x', rfw:{library, root:'root'}, data:{head:{title:'a'}}}`; `view.update {viewId:'x', data:{rows:[]}}`; `view.update {viewId:'x', rfw:'texto solto'}` | A tab with view id `x` exists, holding that library. Data keys now include both `head` and `rows` (top-level merge). `rfw` as a plain string → error |

#### group "os blocos de lista do OrbStack"

Pane size 700×900.

| ID | Given | When | Then |
|---|---|---|---|
| plugins:orbstack:arvore | `list flat` with: `learning` (avatar `{icon:stack,color:blue}`, status green, strong, `expanded:true`, `toggle:'recolher'`, action projeto); `db` (avatar `{icon:box,color:cyan}`, indent 1, `meta:':3311'`, action abrir, actions `[{action:parar,icon:stop,tooltip:parar}]`); `solto` (avatar `'box'` as a string, dim, action abrir-solto) | tap the expand arrow; tap `learning`; hover `db`; tap `parar` | stack → layers icon, box → inventory icon (×2). Actions `['recolher','projeto']` (arrow sends toggle, row sends action). `solto`'s avatar x = `learning`'s avatar x (rows without arrows reserve its space); `db`'s avatar is further right (indent). `:3311` is visible, then hidden on hover (actions replace meta). Last action `parar` |
| plugins:orbstack:header-tabs-card | `header{title:'Containers', subtitle:'3 no ar', avatar:{text:'D',color:blue}, status:green, actions:[{action:atualizar,icon:refresh,tooltip:atualizar},{icon:more,tooltip:mais,menu:[{action:limpar,label:'limpar os parados',icon:trash}]}]}`; `tabs{items:[{label:Containers,action:'aba:c',active:true,count:49},{label:Imagens,action:'aba:i'}]}`; `columns[card[progress{label:CPU,detail:'12%',value:0.12,tone:green}], card[input{id:busca,icon:search,placeholder:buscar}]]` | tap atualizar; tap Imagens; tap mais → `limpar os parados`; type `nats` | `Containers` appears twice; `49` and `12%` shown; search icon shown. Actions: `atualizar`, `aba:i`, then `limpar`. Values `{"busca":"nats"}` (fields inside card/columns count) |
| plugins:orbstack:console-em-card | `card[console{id:log,lines:['um']}]` | `view.console('log')` | lines `['um']` (nested consoles are registered) |
| plugins:orbstack:svg-always-selected | list flat with `alwaysActions:true`; item api `avatar:{svg:'<svg …><circle … fill="#4C7DFF"/></svg>'}`, status green, `selected:true`, actions parar and remover; then `section{style:'label',text:'Parados'}` | render; tap remover | One SVG rendered; tooltips parar and remover visible without hover; `Parados` shown; the selected title colour ≠ theme fg; actions `['remover']` |
| plugins:orbstack:blocks-version | — | — | `mxPluginBlocks >= 2` (the `blocks` value sent in initialize; source = 2) |
| plugins:orbstack:spans | console | append `{spans:[{text:'api ',tone:cyan,bold:true},{text:'│ '},{text:'ERROR',tone:red}]}` | `line.text == 'api │ ERROR'`; spans are `(api ,cyan,true)`, `(│ ,null,false)`, `(ERROR,red,false)` |
| plugins:orbstack:openShell-embedded | `_ShellStore`; docker plugin with main and `sessions.create` permission | API `session.openShell {command:'docker exec -it db sh', embedded:true, tag:'e:db'}`; select the tab; `sessions.list` | Returns `{tabId}`. The shell was not placed in the grid (`place=false`). Tab is embedded; `owner == 'docker'` (embedded implies owned). `ownedBy(plugin)` is empty (excluded from the generic owned list). Selecting it doesn't put it in panes. The list item has `embedded: true` |
| plugins:orbstack:bloco-terminal | shell tabs `t1` (owner docker, embedded, wrote `root@db:/# `) and `t2` (owner outro, embedded); docker view `ct.db` with `terminal{tabId:t1,expand:true}` and `terminal{tabId:t2,height:100,empty:'não é seu'}`; 800×1000 | render; close the view tab | One embedded terminal (only the plugin's own); `não é seu` shown for t2; terminal height > 600 (expand instead of the default 360). Closing the view also closes t1: tabs = `['t2']` |

#### group "workspaces e feature/hotfix na API"

| ID | Given | When | Then |
|---|---|---|---|
| plugins:api:workspaces | folder `/repos/infra` (name infra); workspace `ATRIUM` containing it; hotfix `login quebrado`; shell tab t1 linked to the hotfix | API calls and `sessionJson(tab)` | `workspaces.list == [{"id":<ws.id>,"name":"ATRIUM","folders":["/repos/infra"]}]` (no `codeWorkspacePath` key when unset). `featuresOrHotfixes.list[0].kind == 'hotfix'`. `projects.list[0]` has name `login quebrado`, kind `hotfix`. `folders.list[0].workspaces == ['ATRIUM']`. Session JSON: `project` and `featureOrHotfix` = `login quebrado`, `featureOrHotfixKind` `hotfix`, `workspaces` `['ATRIUM']` |

---

### 6.x test/plugin_modal_test.dart

**Setup:**
- `desk`: plugin `wiboor` (name Wiboor, icon terminal, `main:['true']`).
- `contributes.screen:{home:'quadro'}`, command `{id:quadro,title:'wiboor: o quadro'}`.
- Two sessions, focus on right.
- `text(t)` = `[{type:text,text:t}]`.

| ID | Given | When | Then |
|---|---|---|---|
| modal:janela-em-modal:fora-da-grade | desk | `openPluginView(wiboor,'tarefa.1',title:'TASK#1',blocks:text('a'),modal:true)`; update to `b`; open again without the modal flag with `c`; `closeModal()`; update to `d` | `pluginModal == tab`; not in tabs nor panes; focus stays on `direita`. Update returns true, block text `b`. Reopen (even without `modal`) is the same tab, still modal, still not in tabs. After close the modal is null and update returns false (→ `view.update` `{open:false}`) |
| modal:janela-em-modal:dock | modal open | `dockModal()`; update; reopen with `modal:true` | Modal null; tab in tabs and panes; focus is the tab. Update returns true. Requesting modal again doesn't modal it (it stays a pane) |
| modal:janela-em-modal:um-por-vez | — | open `tarefa.1` modal, then `tarefa.2` modal; update tarefa.1; close tarefa.2 | Modal = two; tarefa.1 update false (replaced, i.e. closed). Close tarefa.2 → modal null, no plugin-view tabs |
| modal:janela-em-modal:desenho | modal layer 1200×900 | render; Esc; reopen and click at (4,4); reopen and click the content; click `abrir como painel` | Shows `o conteúdo` and `TASK#1`. Esc closes. Click outside closes. Click inside keeps it open. `abrir como painel` docks: modal null, tab in tabs |

---

### 6.x test/plugin_screen_test.dart

**Setup:**
- `board({screen, main})`: wiboor plugin, icon terminal, `main` default `['true']`, `contributes.screen` (omitted when null), command `quadro`.
- `git`: no main, icon terminal, command `status` "git: status" `run:'true'`.
- `desk`: left and right sessions, split `[0.3,0.7]`, focus right.
- `open(store, viewId)`: opens a wiboor view titled with the view id, empty blocks.

#### group "o manifesto"

| ID | Given | When | Then |
|---|---|---|---|
| screen:manifesto:true-ou-home | `screen:true`; `{home:'quadro'}`; null | read from disk | true → `screen` true, `screenHome` null. home → true and `'quadro'`. null → false |
| screen:manifesto:recusas | `{home:'outro'}`; `'sim'`; `screen:true` with no main (commands have `run`) | read | All format errors. Source texts: `"contributes.screen.home" aponta pro comando "outro", que o manifesto não declara`; `"contributes.screen" é true ou um objeto com "home": {"home": "quadro"}`; `"contributes.screen" precisa de "main" pra abrir as janelas` |

#### group "a tela do plugin"

`openPanes` means the tabs visible in the current pane tree.

| ID | Given | When | Then |
|---|---|---|---|
| screen:tela:terminal-owned | screen:true; grid view `hosts`; owned shells ssh-1 then ssh-2 (owner wiboor, loose folder) | select ssh-1; focus grid; select ssh-2; `showSidebarView(null)` | After ssh-1: screen `wiboor`, shownPlugin wiboor, panes `[grid, ssh-1]`. With the grid focused, ssh-2 replaces ssh-1: `[grid, ssh-2]`. Back to sessions: `[left, right]` |
| screen:tela:release-return | screen:true; views `quadro` and `tarefa-1` | `releaseView(tarefa)`; `showSidebarView('wiboor')`; `open('tarefa-1')`; `returnView(tarefa)`; `showSidebarView(null)` | After release: screen null, shownPlugin null, panes `[left, right, tarefa]` (right of the focused pane), focused tab = tarefa, `screenOf(tarefa)` null. Plugin screen: `[quadro]`. Re-open tarefa-1 goes to the sessions screen: screen null, panes `[left,right,tarefa]`, no duplicate. After return: screen `wiboor`, panes `[quadro, tarefa]`. Sessions: `[left,right]` |
| screen:tela:abre-e-volta | screen:true | open quadro; open tarefa-1; `showSidebarView(null)`; `showSidebarView('wiboor')` | quadro: screen `wiboor`, shownPlugin wiboor, panes `[quadro]` (full screen), right not open. tarefa → `[quadro, tarefa]`. Back: screen null, **same** pane tree object, weights `[0.3,0.7]`, focus right, `[left,right]`. Return: `[quadro,tarefa]`, focus on tarefa (the screen keeps its own focus) |
| screen:tela:select-sessao | quadro open | `select(left)` | screen null, shownPlugin null, `[left,right]`, focus left |
| screen:tela:plugin-sem-tela | wiboor (screen) plus git | open quadro; `showSidebarView('git')`; open quadro; select right | git: screen null, shownPlugin git, `[left,right]`. Open quadro: screen `wiboor`, shownPlugin wiboor. Select right: shownPlugin null (not back to git) |
| screen:tela:cycle | quadro and tarefa | `cycle(1)`; select right; `cycle(1)` | First cycle stays in the screen: focused = quadro. After selecting right, cycle → screen null, focused = left (wraps); tarefa not open |
| screen:tela:sem-tela-na-grade | `screen:null` | open quadro | screen null; panes `[left,right,quadro]` |
| screen:tela:fechar-guardada | quadro open (sessions grid stored) | `closeTab(right)`; `showSidebarView(null)` | Plugin screen unchanged `[quadro]`; sessions `[left]` (no hole), focused left |
| screen:tela:fechar-ultima | quadro open | `closeTab(quadro)`; `showSidebarView(null)` | screen still `wiboor`, panes null (empty screen). Sessions `[left,right]` |
| screen:tela:pin | left pinned; quadro open | `showSidebarView('wiboor')`; `showSidebarView(null)` | left still pinned / `isPinned(left)` true (pinned panes off-screen aren't cleaned up) |
| screen:tela:desligar | quadro open | `setPluginEnabled(wiboor,false)` | screen null; shownPlugin null; `[left,right]` |
| screen:tela:config | quadro open; `showSidebarView('wiboor')` | wait 600 ms (save debounce); read the config file | `layout.panes[*].label == ['esquerda','direita']` (plugin views not saved); `layout.tree` not null; `layout.focused == 1`; top-level `sidebarView == 'wiboor'` |
| screen:tela:tela-vazia | `board()` (home quadro); quadro opened then closed | render the pane area | Text `nada aberto do Wiboor` and a button `wiboor: o quadro` (the home command title) |

#### Top-level

| ID | Given | When | Then |
|---|---|---|---|
| screen::icone-roda-home | `board(main:['./plugin.sh'])` with the view.open fixture; `onCall = PluginApi.handle` | `showSidebarView('wiboor')`; poll until panes are set; back to sessions; to wiboor again | Screen `wiboor` immediately. The `home` command is invoked (process starts, `command.invoke` arrives), the plugin calls `view.open quadro`, panes become `['quadro']`. Sessions `[left,right]`. Returning: still `['quadro']`, the home command isn't re-run and no second view opens |

---

### 6.x test/floats_test.dart

rfw library: `Row[Text(data.clock), IconButton(icon:"play", tooltip:"iniciar", onPressed: event "toggle" {})]`. Plugin `pomodoro` with main.

| ID | Given | When | Then |
|---|---|---|---|
| floats:flutuante:show-update-hide | store plus API | `float.update {id:'timer',data:{clock:'25:00'}}` before show; `float.show {id:'timer',width:180,height:1000,corner:'topLeft',rfw:{library},data:{clock:'25:00'}}`; update `{phase:'foco'}`; `float.show {id:'x',corner:'meio'}`; `float.show {}`; `float.hide {id:'timer'}` | First update → `{"shown":false}`. Show → `{"shown":true}`; one float shown, width 180, height clamped to max 320, start corner topLeft, library stored. Update merges data: `{clock:'25:00',phase:'foco'}`. Bad corner → error; missing id → error. Hide → none shown |
| floats:flutuante:config | floats loaded from `{'pomodoro/timer':{corner:bottomRight,dx:8,dy:120.4}, 'x/y':{corner:'meio',dx:1,dy:1}, 'z/w':'nada'}` | `toJson()` | Only `pomodoro/timer` kept; `toJson == {"pomodoro/timer":{"corner":"bottomRight","dx":8.0,"dy":120.0}}` (dy 120.4 rounded to a whole pixel, 120.0); invalid entries dropped |
| floats:flutuante:drag | float `timer` 160×50, data `clock 24:59`, overlay 800×600 | render; clear spots; drag toward the top-left (to within about 20 px of the edges); tap `iniciar`; disable the plugin | `24:59` shown; initial x > 800−160−8 (bottom-right with 8 px margin). After the drag: spot = topLeft, dx 8.0, dy 8.0 (snaps to both edges within the threshold). A tap on the button doesn't change the spot. Disabling the plugin hides the float, but the spot is kept for its return |

---

### 6.x test/suggestion_plugins_test.dart

`manifest({suggestions})`: `id tracker`, `main ['./plugin.sh']`, `permissions ['sessions.create']`, `contributes.suggestions` (default `{label:'criar a task no Wiboor'}`, omitted when null). No activationEvents, so it activates on startup.

#### group "contributes.suggestions"

| ID | Given | When | Then |
|---|---|---|---|
| sugg:contrib:label | default | parse | `suggestions == 'criar a task no Wiboor'`; no warnings |
| sugg:contrib:sem-main | `main:null` | parse | Format error (`"contributes.suggestions" precisa de "main": quem recebe a sugestão é o processo`) |
| sugg:contrib:formato | `suggestions:true`; `{label:' '}` | parse | Both format errors (`"contributes.suggestions" é um objeto com "label": {"label": "criar a task no …"}`) |
| sugg:contrib:ausente | null | parse | `suggestions` null |

#### group "suggestion.start num processo de verdade"

**Setup:**
- `_taker` fixture at `<tmp>/tracker`.
- `onCall` records calls and returns `{tabId:'nova'}`. Teardown runs `stopAll`.

| ID | Given | When | Then |
|---|---|---|---|
| sugg:real:created | setup | `suggestionTakers`; `offerSuggestion(tracker, {suggestion:{id:'task_00000001',title:'t',tldr:'',prompt:'p'}, choice:'here', origin:{tabId:'a',root:'/repo',isRepo:true}})`; poll | Takers = `['tracker']`. Exactly one call; it starts with `suggestion.created` and contains `"suggestionId":"task_00000001"` and `"ref":"TASK#1"` |
| sugg:real:ponta-a-ponta | `storeWithOne`; store plugins = the scanned ones; `onCall = PluginApi.handle`; tab a holds a suggestion `{id:'task_00000001',title:'x',tldr:'',prompt:'o pedido'}`; terminal output captured | `startSuggestionInPlugin(store.suggestionTaker, a, task, here)`; poll until the queue is empty; wait 300 ms | The suggestion leaves a's queue. Captured terminal output contains `Vamos trabalhar na TASK#1` (intro) and `o pedido` (prompt), i.e. for "here" the intro plus prompt is typed into the origin session |
| sugg:real:desligado | setup | `enabled = false` | `suggestionTakers` empty |

#### group "o fim do caminho no store"

| ID | Given | When | Then |
|---|---|---|---|
| sugg:store:sem-processo | in-memory taker (no process running); `store.suggest('a',…)` | `startSuggestionInPlugin(...)` | Delivery fails quietly; the suggestion stays in a's queue |
| sugg:store:resposta-orfa | a suggestion queued but never delivered | `suggestionTracked(taker, suggestionId: task.id, ref:'TASK#1')` | Returns null (the API answer for `suggestion.created`); the suggestion stays queued |

#### group "o switch no cartão"

| ID | Given | When | Then |
|---|---|---|---|
| sugg:card:sem-taker | no plugins; a suggestion `Converter SQL` | render the card | No switch |
| sugg:card:com-taker | taker plugin added | render; tap the label text | Text `criar a task no Wiboor`; `suggestionsToPlugin` false, then true after tapping the label (the whole row toggles). Tooltip matches regex `^cria a tarefa no tracker, a worktree dela` |
| sugg:card:avulsos | tab in the loose folder (cwd /tmp); switch already on | render | Switch present; no `iniciar com worktree`; tooltip `cria a tarefa no tracker e abre a sessão nesta pasta` |

---

### 6.y Other tests touching plugins

**`test/quick_input_test.dart`**, group "o campo de texto rápido (window.input)"
- **quick-input:enter:** title `nome do grupo`, placeholder `trabalho, pessoal…`. Typing `  AWS  ` and pressing enter returns `AWS` (trimmed).
- **quick-input:vazio-esc:** whitespace + enter → null; Esc → null.
- **quick-input:valor:** prefilled `Staging` is fully selected (0..7).
- **quick-input:api:** with no `quickInput` stub, `window.input {title:'x'}` errors. With a stub, `window.input {placeholder:'nome', value:'a'}` returns `'AWS'`. The stub received `{title:'SSH', placeholder:'nome', value:'a', prompt:null}`: the title defaults to the plugin **name**.

**`test/quick_input_test.dart`**, group "o formulário em modal (window.form)"
- **The form:** title `novo host`, fallback title `SSH`.
  - Fields:
    - `name` "nome"
    - `host` "host", required
    - `user` "usuário", half
    - `port` "porta", half, value 22 (a number)
    - select `group` "grupo", value AWS, options `[{value:'',label:'sem grupo'},'AWS',{value:'::novo',label:'novo grupo…'}]`
    - `newGroup` "nome do novo grupo", required, `showIf {field:group, value:'::novo'}`
  - Buttons: apagar (danger), conectar "salvar e conectar", salvar (primary).
- **form:obrigatorio:**
  - The showIf field is hidden, and the two half fields share a row.
  - Clicking `salvar` with host empty shows `falta host` and the modal stays open.
  - Typing `exemplo.com` and pressing enter submits through the primary button: action `salvar`, values `{name:'',host:'exemplo.com',user:'',port:'22',group:'AWS',newGroup:''}`. All values are strings, including hidden fields.
- **form:showIf:**
  - Choosing `novo grupo…` reveals `nome do novo grupo`.
  - `salvar e conectar` then shows `falta nome do novo grupo`.
  - After filling `Clientes`: action `conectar`, `group '::novo'`, `newGroup 'Clientes'`.
  - Source text: `falta ${label ?? 'este campo'}`.
- **form:danger-error-cancel:**
  - The `error` text `a porta é um número de 1 a 65535` is displayed.
  - `apagar` (danger) submits even with the required field empty.
  - `cancelar` → null.
- **form:api-malformado:** `window.form {fields:[]}` → error; a field with `type:'data'` → error; `{fields:[{id:'a'}]}` → `{"action":"ok","values":{}}`.

**`test/rail_badge_test.dart`** (ssh plugin with icon and a sidebar command; `sidebarBadges['ssh']` set)
- `▶` badge: width < 24 (sized to its text).
- `três arquivos` badge: width ≤ rail width − 4 (capped).

**`test/reveal_test.dart`**, "lateral escondida volta, e a aba de plugin volta pra lista": with the sidebar hidden and the sidebar view set to `algum-plugin`, revealing a tab unhides the sidebar and resets the sidebar view to the sessions list.

**`test/shortcuts_test.dart`**, "o meta de um manifesto de plugin vira Ctrl+Shift": the parsed chord `meta+alt+g`, mapped for the host (Linux), becomes ctrl+shift+alt+G; `ctrl+alt+g` is unchanged.

**`test/theme_test.dart`**, "catppuccin comes from its plugin…": without a plugin, `catppuccin-mocha` and `catppuccin-frappe` resolve to the default and are not built-in. After adding a plugin palette with `id catppuccin-mocha`, `byId` of `catppuccin-mocha`, `catppuccin-frappe` and `rose-pine` all return it (retired aliases redirect to plugin themes).

**`test/search_test.dart`**: only a comment ("the report is a plugin now; no button without it"); no plugin test.

---

### 6.z Tested behaviour not documented in docs/plugins.md

1. **Manifest parsing details.**
   - `main` may be a single string and becomes a one-element list.
   - Themes may be plain strings (`"themes":["t.json"]`); the docs show only `{path}`.
   - `name` falls back to `id`.
   - The exact error texts are listed in the tables above.
   - Unknown `contributes.*` keys produce the warning `"contributes.<k>" não existe nesta versão — ignorado` (in source; the example-manifest test checks there are no warnings).
2. **Unparseable key strings** (`banana+z+`) become a warning and the command keeps no key. The docs only cover keys already in use and reserved keys.
3. **Scan rules.**
   - `.staging` is skipped.
   - Duplicate ids: the first by directory order wins and the rest are `invalid` with `outro plugin já usa o id "<id>"`.
   - A rejected theme logs the reason and does not invalidate the plugin.
   - Persistence shape: `{"disabled":[ids]}` and `{"settings":{id:{k:v}}}`; empty config → `{}`.
4. **Install internals.**
   - Staging, then commit or discard.
   - `replacing` holds the old version.
   - Into the single subfolder of a package, ignoring `__MACOSX`.
   - Install error mentioning `maestria-plugin.json`.
   - Linking the same dev folder twice is an error.
   - Uninstalling a linked plugin removes only the symlink.
5. **Process.**
   - The failure reason for a missing executable is `não achei "<x>" — nem na pasta do plugin, nem no PATH`.
   - Invalid stdout lines are logged with the prefix `stdout: `.
   - A stopped process returns to `idle`, and `restart` exists.
6. **Dead-key matching.** A chord also matches by physical key position (macOS ⌥ dead keys).
7. **Sidebar commands.**
   - The cap is 3 (documented), but which commands land in the rail versus the footer depends on the plugin having an `icon`. Rail membership comes from: having an icon with commands, having `contributes.sidebar`, or owning a terminal.
   - The rail order is stored in config, can be reordered by drag, and keeps its place while a plugin is disabled.
   - Clicking the already-shown tab hides the sidebar.
   - Disabling the shown plugin resets the sidebar view.
   - The plugin's name prefix (`flutter: `) is stripped from command titles in its generic tab.
   - Plugin views are attached to the loose folder but excluded from the sessions and loose lists.
8. **`sidebar.update` details.** A numeric `badge` is stringified, and loading shows the text `Abrindo <Name>…` / `<Name> caiu: <crash>` (the docs only say "abrindo…").
9. **Owned terminals.** When the owner plugin is removed, its terminals go back to the loose list. Embedded terminals are excluded from the owner's generic list and are not placed in the pane grid.
10. **Settings.**
    - `resolve` falls back to the default on a wrong type or a select value not in the options.
    - A select without a default uses its first option; a boolean without a default is false.
    - `pluginColor` rejects short hex and unknown names.
11. **`window.pick`.**
    - Errors with no window.
    - Items without `value` are dropped.
    - `label` defaults to `value`.
    - `title` is passed through.
    - `window.input`'s title defaults to the plugin name.
12. **`window.form`.**
    - Errors on empty `fields` or an unknown field type.
    - Values are always strings (a numeric `value` 22 becomes `"22"`).
    - The missing-field text is `falta <label>`; the cancel button is labelled `cancelar`.
13. **Blocks and views.**
    - The unknown-block label is exactly `Bloco desconhecido: <type>` (the docs say "bloco desconhecido").
    - The missing-plugin banner reads `O plugin não está mais instalado`.
    - The view subtitle is `Plugin · <name>`, and `view.revision` exists.
    - Actions without an icon are not rendered.
    - A right-click doesn't fire the row action.
    - The header close counts as an icon button.
    - A `card` console is still registered.
14. **rfw.**
    - The parse-error text is `a interface do plugin não leu:\n<e>`.
    - Menu choices arrive through `onMenu` with the event name as the action.
    - Nested DropTargets: the innermost one that accepts the payload wins.
    - A target sizes to its child under unbounded constraints.
    - `rfw` given as a plain string in `view.update` is rejected.
    - Menu `children` items use `color` as an integer (0 = outline only).
15. **Floats.**
    - Config key `<pluginId>/<floatId>`, stored as `{corner,dx,dy}`.
    - Rounding (120.4 → 120.0) is untested in the docs.
    - The snap margin is 8 px and the default first position is bottom-right with an 8 px margin.
    - `float.show` without an `id` or with a bad corner errors.
    - The position survives the plugin being disabled.
16. **Screen.**
    - Owned terminals replace the other terminal when the grid view has focus.
    - `cycle` stays within the current screen.
    - Selecting a session from a plugin screen resets the sidebar to sessions, not to the previously shown non-screen plugin.
    - Closing the last view leaves the screen empty (`panes` null) instead of leaving.
    - Pinned panes survive.
    - Disabling the plugin returns you to sessions.
    - Config saves only the sessions grid (`layout.panes[].label`, `layout.tree`, `layout.focused`, top-level `sidebarView`).
    - Empty-screen text `nada aberto do <Name>`, with a button titled by the home command.
    - The home command is not re-run when the screen already has views.
17. **Modal.**
    - Re-opening the same view without `modal` keeps it modal.
    - Closing via `closeModal` makes later updates return false.
    - `view.close` closes the modal.
    - The "abrir como painel" label is verbatim.
18. **Suggestions.**
    - Delivery to a plugin that isn't running fails quietly and keeps the card.
    - A `suggestion.created` for an id that was never delivered returns null and is ignored.
    - For `here`, the intro and the prompt are written to the origin session's terminal.
    - Card tooltips: `cria a tarefa no tracker, a worktree dela…` and `cria a tarefa no tracker e abre a sessão nesta pasta`. In the loose folder the worktree option is hidden.
19. **Themes.** Retired aliases (`catppuccin-frappe`, `rose-pine`) redirect to a plugin theme with id `catppuccin-mocha`.
20. **Command expansion.** Unknown `${var}` is left literal, and quoting uses `'\''` escaping.
21. **Platform caveat.** `plugins:manifesto:tecla-invalida` expects `meta` kept as meta and a `⌘Q` label, while the source maps meta → Ctrl+Shift on Linux (`MxChord.linux = Platform.isLinux`). That expectation is macOS-only and conflicts with `shortcuts_test` on Linux.

### 6.t Timeouts and durations

**Used in tests:**
- **Polling:** 100 iterations × 30 ms ≈ 3 s, in `until` and the inline loops.
- **Negative checks:** fixed 100 ms waits (plugin should not start; no restart attempt).
- **Suggestion end-to-end:** an extra 300 ms wait for terminal output.
- **Config save debounce:** waits 600 ms before reading the config file.
- **UI pumps:** sidebar renders use 200 ms; widget teardowns pump 1 s; the rail drag runs over 400 ms.

**Implemented in source:**
- `initialize`: 10 s.
- Requests: 30 s by default.
- Shutdown grace: 800 ms before SIGTERM.
- Float height: 32–320 (docs: width 80–480, default 200×64).
- Console `max` default: 5000.
- Embedded terminal default height: 360.
- `maxSidebarCommands`: 3.
- `mxPluginBlocks`: 2.

---

## 7. Suggested Rust module layout (each file ≤ 400 LOC)

Crate split keeps the RFW engine UI-toolkit-agnostic (parse + evaluate to a widget tree), with a thin
renderer per toolkit.

```
crates/
  mx-plugin/                     # host-side plugin system, no UI
    src/lib.rs                   # re-exports, constants (API=1, BLOCKS=2, MANIFEST_NAME)
    src/manifest/mod.rs          # PluginManifest struct + parse() entry, field order/errors (§1.1)
    src/manifest/commands.rs     # contributes.commands, CommandTarget, key parsing → warnings
    src/manifest/settings.rs     # PluginSetting, SettingType, resolve()
    src/manifest/contributes.rs  # themes, sidebar, screen, suggestions, unknown-key warnings
    src/chord.rs                 # chord parse/forHost/label/rejection, physical-key fallback (§4.6)
    src/registry.rs              # Plugins: scan(), byId, commands, sidebar commands, busy set
    src/state.rs                 # MxPlugin, PluginState derivation, log ring (400) with HH:MM:SS
    src/persist.rs               # {disabled, settings} load/save, float spots json
    src/install/stage.rs         # git/zip/dir staging, manifest dir discovery, discard
    src/install/commit.rs        # commit, link (symlink), uninstall, data dir removal
    src/process/spawn.rs         # resolve_executable, env, spawn, initialize (10 s), onReady
    src/process/conn.rs          # PluginConnection: line framing, pending map, timeouts, close()
    src/process/lifecycle.rs     # start dedup, crash handling, restart, stop_all/kill_all
    src/rpc/types.rs             # RpcError codes/messages, Message enum (serde_json::Value based)
    src/rpc/dispatch.rs          # method → handler table; permission gate; param helpers
    src/rpc/window.rs            # window.*, clipboard, settings.get, editor.open, command.busy
    src/rpc/sessions.rs          # sessions/folders/workspaces/features/chats, session.* , terminal.sendText
    src/rpc/views.rs             # view.*, sidebar.update, float.*, pane.openMarkdown, rfw/data parsing
    src/rpc/suggestions.rs       # suggestion.start / suggestion.created tracking
    src/events.rs                # emit(type, data, activations, needs), tell, wake, viewAction
    src/session_json.rs          # session JSON shape (§2.5), status names/labels
    src/view.rs                  # PluginView, setRfw merge, setBlocks + console adoption
    src/console.rs               # PluginConsole lines/spans/max
    src/form.rs                  # PluginForm parse + validation (§4.4)
    src/floats.rs                # PluginFloats show/update/hide, clamps, spot snapping math
    src/expand.rs                # ${var} expansion + shell quoting
    src/theme.rs                 # plugin theme JSON → palette, built-in/alias collision rules
    tests/…                      # one file per §6 group; fixtures/ with the sh scripts verbatim

  mx-rfw/                        # RFW text format: parser + evaluator (toolkit-agnostic)
    src/lib.rs
    src/lexer.rs                 # tokens, comments, strings/escapes, numbers (int vs double!)
    src/parser.rs                # library: imports, widget decls (+state), constructors
    src/ast.rs                   # Library, WidgetDecl, Node (Map/List/Loop/Switch/Ref/Event/Set…)
    src/value.rs                 # DynamicValue (Int/Double/Bool/String/List/Map/Missing)
    src/content.rs               # DynamicContent: top-level key merge, `theme` injection
    src/eval.rs                  # reference resolution (data./args./state./loop vars), loops, switch
    src/events.rs                # event "name" {args} → (name, plain JSON), extra-arg merging
    src/args.rs                  # ArgSource: typed getters v<T> with strict int/double semantics
    src/decoders/mod.rs          # color, edgeInsets, alignment, enums (§3.x)
    src/decoders/decoration.rs   # boxDecoration, border, borderRadius, boxShadow, gradient
    src/decoders/text.rs         # textStyle, fontWeight, textAlign, overflow
    src/widgets/core.rs          # catalog of core.widgets used (§3) → neutral WidgetNode
    src/widgets/material.rs      # catalog of core.material used
    src/widgets/maestria.rs      # Svg, Glyph, IconButton, Tooltip, Pressable, MenuButton, Draggable, DropTarget
    tests/…                      # golden parse tests on the 7 real .rfwtxt files

  mx-ui-plugins/                 # renderer in the chosen Rust UI toolkit
    src/blocks/mod.rs            # dispatcher + unknown-block fallback, expanding layout
    src/blocks/basic.rs          # heading/text/markdown/code/divider/section/kv/progress
    src/blocks/controls.rs       # button/row/input/checkbox/select + field state sync (§3.2)
    src/blocks/list.rs           # list rows, avatar, actions, menu, tree spacing
    src/blocks/layout.rs         # header/card/tabs/columns
    src/blocks/console.rs        # console view (follow logic), terminal embed
    src/blocks/calendar.rs       # calendar widget + pt-BR strings
    src/rfw_render.rs            # WidgetNode → toolkit widgets
    src/drag.rs                  # drag payload/targets, nested target resolution
    src/icons.rs                 # icon name table, PluginGlyph (svg in plugin dir, tint)
    src/tones.rs                 # tone → palette color
    src/pane.rs                  # framed/bare/modal containers, header, crashed strip
    src/modal.rs                 # modal layer
    src/float_layer.rs           # float layer, drag/snap
    src/dialogs/{install,trust,log,pick,input,form,settings}.rs
```

Implementation notes:
- Keep JSON as `serde_json::Value` at the protocol boundary; many fields are loosely typed and errors
  must match the Dart behavior (e.g. wrong types silently ignored vs rejected) — see §1.1/§2.4.
- `serde_json` must preserve the int/double distinction of incoming numbers (`13` vs `13.0`): RFW
  data from plugins arrives as JSON where `13.0` is serialized by JS as `13` → an int; RFW `v<double>`
  returns null for ints (that is why docs say sizes go in the library text, not in data).
- Serialize host messages compactly; integer request ids from 1.
- Process I/O on a tokio task per plugin: stdout line reader, stderr line reader → log, exit watcher.
