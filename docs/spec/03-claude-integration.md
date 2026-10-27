# 03 — Claude Code integration (hooks, MCP, status line, usage, notifications, setup, reader)

Source of truth (READ-ONLY): `~/maestria/repo`
`lib/services/{hooks,task_mcp,usage,notify,setup,docs}.dart`, `lib/ui/{setup_pane,suggestions,doc_pane}.dart`,
the hook/notice/suggestion parts of `lib/services/store.dart` and `lib/models.dart`.

Conventions in this document:

- "UTF-16 length" — Dart `String.length` counts UTF-16 code units. Every length cap / clip below is in
  UTF-16 units (use `s.encode_utf16().count()` and clip on a UTF-16 boundary, never splitting a surrogate pair
  unless you want byte-exact Dart behaviour; Dart *does* split pairs — harmless in practice).
- `clip(text, max)` (reducer helper) = collapse every run of whitespace (`\s+`) to one space, `trim()`;
  empty → `null`; if UTF-16 length ≤ max return it, else `first max units + "…"` (U+2026, no trim after cut).
- `Sh.q(v)` = `'` + v with every `'` replaced by `'\''` + `'` (POSIX single-quote).
- `Sh.run(cmd)` = spawn `[$LOGIN_SHELL, "-lc", cmd]` with the app env (PATH augmented, see shell spec);
  `ok` ⇔ exit code 0; `ProcessException` → code -1. `$LOGIN_SHELL` = `/bin/zsh` on macOS, else `$SHELL` if
  it exists, else first existing of `/bin/bash`, `/usr/bin/bash`, `/bin/zsh`, `/usr/bin/zsh`, else `/bin/sh`.
- `$STATE` = `$HOME/.maestria` (`mxStateDir`; `HOME` fallback `/`). Under tests Dart used
  `$TMP/maestria-test-$pid` — Rust should take the state home as an injectable parameter.
- JSON objects are emitted in insertion order (enable `serde_json/preserve_order`). Dart `jsonEncode` does not
  escape `/` nor non-ASCII — same as serde_json.

---

## 1. Hook server (`HookServer`)

### 1.1 Transport

One loopback HTTP/1.1 server per app instance, bound to **`127.0.0.1` (IPv4 only)**. It multiplexes three
tenants by the first path segment:

| Path | Method | Tenant |
|---|---|---|
| `/mcp/<tabId>` | POST only (others → `405`, empty body) | MCP server (§2) |
| `/status/<tabId>` | any | status-line snapshots → `statuses` stream (§3.1) |
| anything else, i.e. `/hook/<tabId>` | any | Claude Code `http` hooks → `events` stream |

`tabId` = second path segment (`segments[1]`), or `""` when there are < 2 segments. Only the first segment
is inspected; `/whatever/tab1` is treated exactly like `/hook/tab1`.

Hook / status request handling (exact):

1. Read whole body as UTF-8.
2. Empty body → payload `{}`; else JSON-decode and require a JSON **object** (array/scalar → exception).
3. `name = payload["hook_event_name"]` if string, else `""`.
4. If first segment is `status`: if `tabId != ""` emit `StatusEvent(tabId, payload)`. (It never goes through the
   hook reducer.)
5. Else if `tabId != ""` and `name != ""`: emit `HookEvent(tabId, name, payload)`.
6. Any exception in 1–5 is swallowed ("a malformed body must never stall the session").
7. **Always** respond `200`, `Content-Type: application/json; charset=utf-8`, body exactly `{}` (an empty
   decision — Maestria observes, never rules on a tool call).

Both streams are broadcast (multi-subscriber). The store subscribes `events → applyHook`,
`statuses → applyStatus` and sets `desk = store` (the MCP backend) right after `start()`.

### 1.2 Port stability (`$STATE/hook-port`)

The port is baked into every launched session's `--settings`/`--mcp-config` and Claude Code's daemon respawns
sessions with identical flags, so the port must survive app restarts.

`start()` (idempotent; no-op if already running):

1. `saved()` = read `$STATE/hook-port`, `trim()`, parse int; valid only if `1024 < port < 65536`; any
   read/parse failure → none.
2. If saved: try bind `127.0.0.1:saved`. On bind failure (in use) fall through.
3. Bind `127.0.0.1:0` (ephemeral).
4. `remember(port)`: if `saved() != port` → `mkdir -p $STATE`, write `"<port>\n"`. Write errors ignored.
5. `writeStatusLine()`: `mkdir -p $STATE`, (over)write `$STATE/statusline.sh` with `statusLineScript` (§3.1)
   **on every start** (sessions reference it by path). No chmod — it is invoked as `sh <path>`. Errors ignored.

`stop()` closes the listener forcibly and clears state. `port` = bound port or 0.
(Rust `std`/tokio `TcpListener::bind` sets `SO_REUSEADDR` on Unix, matching Dart's ability to rebind a
TIME_WAIT port.)

### 1.3 How hooks are installed — **nothing is written to Claude Code's settings files**

Maestria never edits `~/.claude/settings.json` or any project settings. Each Claude panel is launched as
(`store._launchClaude`, joined with single spaces, run in the panel's pty via the login shell):

```
claude --mcp-config <Sh.q(mcpConfigFor(tabId))> --name <Sh.q(name)> --settings <Sh.q(settingsFor(tabId, cwd: tab.cwd))> [--append-system-prompt <Sh.q(brief)>] [--resume <Sh.q(resumeId)>] [-- <Sh.q(prompt)>]
```

- `--mcp-config` MUST come first (it is variadic and would swallow the positional prompt; `--name` terminates it).
- `name` = panel label, or basename of cwd. `brief` = project ("feature/hotfix") brief, trimmed, only if non-empty.
  `prompt` = trimmed opening prompt, only if non-empty (positional, last, after `--` so a prompt starting with `-` is not read as an option).
- Display line shown in the terminal instead of the real command:
  `claude --mcp-config <tarefas :PORT> --name 'NAME' --settings <hooks :PORT>` + ` --append-system-prompt <briefing de PROJECT>` (if brief) + ` --resume …` (if resume) + ` <prompt inicial>` (if prompt).
- 3 s after launch, if the tab still exists and has not exited: `hooks.settle()` (§1.6) and notify listeners —
  Claude Code does not deliver `SessionStart` to `http` hooks, so nothing else announces the prompt is ready.

Tab ids (they are in the URLs): `"tab" + run + "_" + seq` where `run` = app start time in microseconds since
epoch, base-36, and `seq` an incrementing int. The per-run stamp prevents a session that outlived a previous app
run from painting a new panel that reused `tab2`.

#### `settingsFor(tabId, cwd, home=$HOME)` — exact JSON (compact, this key order)

```json
{
  "permissions": {"allow": ["mcp__maestria__suggest_task", "mcp__maestria__dismiss_task"]},
  "hooks": {
    "SessionStart":       [{"hooks": [{"type": "http", "url": "http://127.0.0.1:PORT/hook/TAB", "timeout": 5}]}],
    "SessionEnd":         [{"hooks": [ ...same... ]}],
    "UserPromptSubmit":   [{"hooks": [ ...same... ]}],
    "PreToolUse":         [{"matcher": "*", "hooks": [ ...same... ]}],
    "PostToolUse":        [{"matcher": "*", "hooks": [ ...same... ]}],
    "PostToolUseFailure": [{"matcher": "*", "hooks": [ ...same... ]}],
    "Notification":       [{"hooks": [ ...same... ]}],
    "Stop":               [{"hooks": [ ...same... ]}],
    "StopFailure":        [{"hooks": [ ...same... ]}],
    "SubagentStop":       [{"hooks": [ ...same... ]}]
  },
  "statusLine": {
    "type": "command",
    "command": "sh 'STATE/statusline.sh' 'http://127.0.0.1:PORT/status/TAB' 'B64'",
    "padding": <copied verbatim from the user's statusLine, key omitted if absent>
  }
}
```

- `matcher` key appears **before** `hooks` and only for the three tool events. `timeout` is the integer `5`.
- `command` = `"sh " + Sh.q(statusLineFile) + " " + Sh.q(statusUrl) + " " + Sh.q(encoded)`; `encoded` is
  `base64(utf8(userCommand))` (standard alphabet, padded) or `""` → the command ends in ` ''`.
- `userStatusLine(cwd, home)`: candidates in order `cwd/.claude/settings.local.json`,
  `cwd/.claude/settings.json` (both only if `cwd` non-empty), `home/.claude/settings.json` (if `home`
  non-empty). First file that parses as a JSON object whose `statusLine` is an object with
  `type == "command"` and string `command` wins (returns the whole object; `padding` is read from it).
  Missing file / invalid JSON / non-object → skip.

#### `mcpConfigFor(tabId)` — exact JSON

```json
{"mcpServers":{"maestria":{"type":"http","url":"http://127.0.0.1:PORT/mcp/TAB","alwaysLoad":true}}}
```

It adds to (does not replace) the user's MCP servers — `--strict-mcp-config` is never passed.

### 1.4 Events subscribed

`SessionStart, SessionEnd, UserPromptSubmit, PreToolUse(*), PostToolUse(*), PostToolUseFailure(*),
Notification, Stop, StopFailure, SubagentStop`. Payload keys read: `hook_event_name, session_id,
permission_mode, agent_id, prompt, user_input, tool_name, tool_input, is_interrupt, error,
notification_type, message, last_assistant_message, reason, end_reason, cwd`.

### 1.5 `HookState` (per Claude tab)

| Field | Type / default | Notes |
|---|---|---|
| `status` | `ClaudeStatus` = `starting` | |
| `sessionId`, `permissionMode`, `activeTool`, `lastToolTarget`, `lastMessage`, `lastMessageFull`, `lastPrompt`, `question` | `Option<String>` = None | |
| `toolStartedAt`, `lastEventAt` | `Option<Instant/DateTime>` | |
| `questions`, `prompts`, `tools`, `forksOut` | int = 0 | |
| `forkIds` | ordered set of strings | |
| `touched` | `Vec<String>`, cap `maxTouched = 200` (drop oldest) | persisted in config as `touched` |
| `plans` | `Vec<PlanNote{text, at}>`, cap `maxPlans = 8` | persisted as `plans: [{text, at ISO8601}]` |
| `suggestions` | `Vec<SuggestedTask>`, cap `maxSuggestions = 20` | persisted as `suggestions` |

`ClaudeStatus` (order matters for serialization by name): `unknown, starting, ready, working, tool,
waitingInput, waitingAnswer, waitingPermission, idle, ended`.

| status | label | color token |
|---|---|---|
| unknown | `—` | fgFaint |
| starting | `Abrindo` | purple |
| ready | `Pronto` | green |
| working | `Pensando` | accent |
| tool | `Rodando` | accent |
| waitingInput | `Esperando você` | yellow |
| waitingAnswer | `Pergunta` | yellow |
| waitingPermission | `Permissão` | red |
| idle | `Ocioso` | green |
| ended | `Encerrada` | fgFaint |

- `atRest` ⇔ `idle | waitingInput`. `needsHuman` ⇔ `waitingInput | waitingAnswer | waitingPermission`.
- `callToAction`: waitingPermission → `quer aprovação`; waitingAnswer → `te faz uma pergunta`; else `te espera`.
- `busyForks` ⇔ `forksOut > 0`.
- `settle()`: if `status == starting` → `ready`, return true; else return false (never walks back).
- `plan` = last of `plans` or None.
- `subtitle` (first match):
  1. waitingPermission → `Quer aprovação pra {activeTool}` (Dart prints `null` if None).
  2. waitingAnswer → question None: `Te faz uma pergunta`; else `Pergunta: {question}` + (`questions > 1` ? ` (+{questions-1})` : ``).
  3. waitingInput → `lastMessage ?? "Te espera"`.
  4. tool and activeTool set → `{activeTool}` + (`lastToolTarget` ? `({lastToolTarget})` : ``) + (`toolStartedAt` ? ` {elapsedWholeSeconds}s` : ``).
  5. working → `lastPrompt ?? "Trabalhando"`.
  6. idle → `lastMessage ?? "Pronto"`.
  7. else → `status.label`.
- `PlanNote.headline`: for each line, `trim()` then strip leading `^#+\s*`; first non-empty one → if ≤ 70
  units itself else first 70 + `…`; none → `Plano sem título`.

### 1.6 `HookReducer.apply(state, event, payload)` — exact algorithm

```
s.lastEventAt = now
s.sessionId      = payload.session_id      (if string) else keep
s.permissionMode = payload.permission_mode (if string) else keep
  // Dart throws on a non-string here (event aborted half-applied). Rust: treat non-string as absent.

fork = payload.agent_id
if fork is non-empty string:                      // event fired by a subagent on the parent's hook
    if event == "SubagentStop": forkDone(s, fork)
    else if s.forkIds.insert(fork) (newly added) and s.forkIds.len > s.forksOut:
        s.forksOut = s.forkIds.len
    if event == "PreToolUse":  s.tools += 1
    if event == "PostToolUse": touch(s, payload)
    return                                        // NEVER changes status/activeTool etc.
if event == "SubagentStop": forkDone(s, None); return

match event:
  "SessionStart":
      status = ready; forksClear
  "UserPromptSubmit":
      question = None; questions = 0; prompts += 1; status = working
      lastPrompt = clip(payload.prompt ?? payload.user_input, 70)   // OVERWRITES with None if both absent
  "PreToolUse":
      tn = payload.tool_name
      if tn == "AskUserQuestion": asked(s, payload.tool_input)
      if tn in {"Task","Agent"}:  forksOut += 1
      if tn == "ExitPlanMode":    planned(s, payload.tool_input)
      tools += 1
      status = (tn == "AskUserQuestion") ? waitingAnswer : tool
      activeTool = tn (None if absent); toolStartedAt = now
      lastToolTarget = target(payload.tool_input)                   // may overwrite with None
  "PostToolUse":
      status = working; activeTool = None; toolStartedAt = None; question = None; questions = 0
      touch(s, payload)
  "PostToolUseFailure":
      activeTool = None; toolStartedAt = None; question = None; questions = 0
      status = (payload.is_interrupt == true) ? idle : working      // Esc = turn over, no Stop follows
  "StopFailure":
      status = idle; activeTool = None; toolStartedAt = None
      lastMessage = clip(payload.error if string, 90) ?? keep
  "Notification":
      t = payload.notification_type ?? ""
      "permission_prompt" → status = (activeTool == "AskUserQuestion") ? waitingAnswer : waitingPermission
      "agent_needs_input" → status = waitingInput; lastMessage = clip(payload.message, 70) ?? keep
      "idle_prompt"       → if !status.needsHuman: status = idle
      other               → nothing
  "Stop":
      status = idle; activeTool = None; toolStartedAt = None
      closing = payload.last_assistant_message
      lastMessage = clip(closing, 90) ?? keep
      if closing non-null and closing.trim() non-empty: lastMessageFull = closing.trim()
  "SessionEnd":
      reason = payload.reason ?? payload.end_reason ?? "other"
      status = (reason in {"resume","clear"}) ? ready : ended
      forksClear
  other: nothing (only the three header fields changed)
```

Helpers:

- `forkDone(s, id?)`: if id → `forkIds.remove(id)`; `if forksOut > 0 { forksOut -= 1 }` (floor 0).
- `forksClear(s)`: `forkIds.clear(); forksOut = 0`.
- `asked(s, input)`: input must be object with non-empty array `questions`; `questions = len`; if first element
  is an object: `question = clip(first.header, 24) ?? clip(first.question, 60)` (if first is not an object,
  `question` is left unchanged).
- `planned(s, input)`: input object, `plan` non-blank string; if `plans` non-empty and `plans.last.text == plan`
  (exact, untrimmed) → skip; push `PlanNote{text: plan, at: now}`; if len > 8 remove index 0.
  Captured on **PreToolUse** (a rejected plan never gets a PostToolUse).
- `target(input)`: input object; first of keys `file_path, path, notebook_path, pattern` whose value is a
  non-empty string → its last `/`-segment (may be `""` for a trailing slash). Else if `command` non-empty
  string → `clip(command, 28)`. Else None.
- `touch(s, payload)` (PostToolUse only, never PreToolUse):
  - `tool = tool_name ?? ""`, `input = tool_input` must be object, `cwd = payload.cwd`.
  - `Bash`: `command` must be string; for each path in `shellMarkdown(command)` → `remember(absolute(path, cwd))`.
  - `Write|Edit|MultiEdit` → key `file_path`; `NotebookEdit` → key `notebook_path`; value must be non-empty
    string → `remember(absolute(raw, cwd))`. Any other tool → nothing (reads are deliberately excluded).
- `remember(s, path)`: if already in `touched` → no-op (order = first touch); push; if len > 200 remove index 0.
- `absolute(path, cwd)`: starts with `/` → as is; `~` or `~/…` → `expandHome` (replace `~` with `$HOME`,
  stripping a trailing `/` of HOME; unchanged if HOME empty); cwd empty/None → as is; else strip leading `./`,
  strip one trailing `/` from cwd, return `cwd + "/" + rel`.
- `isMarkdownPath(p)`: lowercase basename ends with `.md`, `.markdown` or `.mdx`.

`shellMarkdown(command) -> Vec<String>` (dedup, order of discovery):

1. `commands(command)`: drop heredoc bodies. Iterate lines (`\n`); when inside a heredoc skip lines until a line
   whose `trim() == delimiter` (that line is skipped too). For each kept line, if it matches
   `HEREDOC = <<-?\s*(?:'([^']+)'|"([^"]+)"|(\w+))` → delimiter = first non-null group. Join kept lines with `\n`.
2. Split the result by regex `[|;\n]|&&` into segments. For each segment:
   - every match of `REDIRECT = \d?>>?\s*(?:'([^']+)'|"([^"]+)"|([^\s'"|;&<>()]+))` → `keep(g1 ?? g2 ?? g3)`
     (`2>&1` does not match because `&` is excluded).
   - `words = segment.trim().split(/\s+/)`; `head` = index of first word not matching `^\w+=` (none → next
     segment); `verb` = last `/`-segment of `words[head]`; `args = words[head+1..]`; `flags` = args starting
     with `-`; `operands` = the others.
   - `tee` → keep every operand. `sed` when any flag starts with `-i` or equals `--in-place` → keep every
     operand. `cp | mv | install` → keep the last operand.
3. `keep(token)`: `plainPath`: trim; if length ≥ 2 and starts with `'` or `"` and ends with the same char →
   strip both; reject empty or containing any of `$ * ? \``; then require `isMarkdownPath`; add if not present.

Use ASCII semantics for `\w` / `\d` (`(?-u:\w)` in the Rust `regex` crate) to match Dart.

### 1.7 Store side of a hook (`AppStore.applyHook`) — order matters

```
tab = byId(e.tabId); if none → return
before = tab.hooks.status; producedBefore = tab.hooks.touched.len
HookReducer.apply(tab.hooks, e.name, e.payload)
plugins.emit("hook", {tabId, name, payload(raw)}, activations ["onHook:<name>", "onHook:*"], needs permission "hooks")
if status changed: plugins.emit("session.status", {tabId, status: after.name, previous: before.name}, activations ["onSession"])
if tab.done and e.name == "UserPromptSubmit": tab.done = false; save
if hooks.sessionId != None and tab.sessionId != hooks.sessionId: tab.sessionId = hooks.sessionId; save
if touched.len changed: save
after = status
if after != before:
    after == waitingAnswer      → notice(tab, question)
    after == waitingPermission  → notice(tab, permission)
    !before.atRest && after.atRest → notice(tab, finished)
    !after.needsHuman && !after.atRest → readNoticesOf(tab.id, notify=false, suggestions=false)
if !before.atRest && after.atRest:
    if followUps non-empty: armed = true
    restedAt = now; unseen = !watching(tab)
else if before.atRest && !after.atRest:
    restedAt = None; unseen = false
checkAlerts()       // §4
notifyListeners()
```

`MxTab.status` = `unknown` for reader/setup/plugin tabs; shell tabs → `ended` if exited else `unknown`;
claude tabs → `hooks.status`.

Notices (the bell; UI in `ui/notices.dart`): `noticeCap = 50` (newest first, insert at 0, drop tail),
`toastCap = 3`, `toastLife = 8 s` (timer only runs while the window is active; hovering holds it),
`watching(tab)` ⇔ window active ∧ `focusedPaneId == tab.id` ∧ the tab is on screen.
`notice(tab, kind, suggestionId?, detail?)`: unless kind is `suggested`, first mark all of that tab's notices read
(no notify); create `MxNotice{tabId, kind, title: tab.title, suggestionId, detail, at: now, read: watching(tab)}`;
toast it if unread ∧ not do-not-disturb ∧ bell list closed. `readNoticesOf(tabId, notify=true,
suggestions=true)` marks unread notices of that tab read and drops their toasts (skipping `suggested` ones
when `suggestions=false`), returns whether any changed. `unreadNotices` = count of unread.
Notice verbs: question `te fez uma pergunta`, permission `quer aprovação`, finished `terminou o trabalho`,
suggested `sugeriu uma tarefa`. A 1 s app timer calls `seeFocused()` (watching the focused tab for a tick ⇒
read its notices / clear `unseen`).

---

## 2. MCP server (`TaskMcp`) — must stay wire-identical

### 2.1 Transport

MCP "streamable HTTP", JSON-only (no SSE, no `Mcp-Session-Id`, no `Accept` check), on the hook
server at `POST /mcp/<tabId>`.

- Every route of the hook server (hooks, status, MCP) answers `403`, empty body, to a request carrying an
  `Origin` header or whose `Host` is not `127.0.0.1:<port>`/`localhost:<port>` (browser CSRF, DNS rebinding).
  Bodies over 64 MiB are refused.

- Non-POST → `405`, empty body.
- Body read as UTF-8; JSON parse failure → answer = parse error. Any other exception while handling → parse error.
- `answer == null` (notification or batch of only notifications/empty batch) → `202`, empty body.
- Otherwise `200`, `Content-Type: application/json; charset=utf-8`, body = `jsonEncode(answer)`.
- If `desk` is not set yet, a no-op desk is used: `suggest → null`, `withdraw → (withdrawn:false, already:null)`.

Server name `maestria` ⇒ tool names as the session sees them: `mcp__maestria__suggest_task`,
`mcp__maestria__dismiss_task` (also pre-allowed in `--settings`).

Constants: `server = "maestria"`, `suggestTool = "suggest_task"`, `dismissTool = "dismiss_task"`,
`maxPrompt = 32000`, `protocolVersion = "2025-06-18"`, task id regex `^task_[0-9a-f]{8}$`.

### 2.2 JSON-RPC dispatch (`handle(body, tabId, desk)`)

- body is an array → map each element through `one`, drop nulls; empty → null, else the array.
- else → `one(body)`.

`one(message)`:
- not an object → `{"jsonrpc":"2.0","id":null,"error":{"code":-32600,"message":"invalid request"}}`
- `id` absent/null → no response (null).
- `method` absent or not a string (with an id) → `{"jsonrpc":"2.0","id":<id>,"error":{"code":-32600,"message":"invalid request"}}`.
- `params` = object or `{}`.
- `initialize` → result:
  ```json
  {"protocolVersion": "<params.protocolVersion if string, else 2025-06-18>",
   "capabilities": {"tools": {"listChanged": false}},
   "serverInfo": {"name": "maestria", "version": "1.0.0"},
   "instructions": "<INSTRUCTIONS>"}
  ```
- `ping` → result `{}`.
- `tools/list` → result `{"tools": TOOLS}`.
- `tools/call` → §2.4.
- other → `error {"code": -32601, "message": "method not found: <method>"}`.

Result envelope: `{"jsonrpc":"2.0","id":<id verbatim>,"result":<result>}`.
Error envelope: `{"jsonrpc":"2.0","id":<id or null>,"error":{"code":C,"message":M}}`.
Parse error: `{"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"parse error"}}`.

### 2.3 Texts (verbatim; `\n` are real newlines)

`INSTRUCTIONS`:
```
This session runs inside Maestria, a desktop cockpit for Claude Code sessions. When you notice work that deserves doing but is outside the scope of the current task, call suggest_task instead of widening the task or only mentioning it in prose: the user gets a card they can start as a separate session with one click.
```

`TOOLS` (exact JSON, this key order):

```json
[
  {
    "name": "suggest_task",
    "title": "Suggest a task",
    "description": "Suggest a separate task for something you noticed that is outside the scope of your current work.\n\nCall this when you come across something that deserves fixing but does not belong in this change: dead code, stale docs, a missing test, a confirmed TODO, a bug or security issue spotted in passing. Do not use it for trivial fixes you can make inline, for anything the user asked you to do, for vague impressions or unverified hunches, or to split off parts of your own task. The call only queues a suggestion and returns; carry on with your current work.\n\nThe user sees a \"tarefa sugerida\" card in the Maestria panel of this session, with your title and tldr, can read the full prompt, and with one click starts it as a new Claude Code session (in a fresh git worktree, or in their checkout of this repository), sends it to this session, or dismisses it. A new session starts from your prompt alone, on a checkout that may not have this session's changes, so the prompt has to stand alone: what to change and why, the files involved by repository-relative path, and any context from this conversation it depends on. If the find involves a secret or credential, say where it is rather than copying the value, since the prompt is stored and displayed.\n\nThe result carries a task_id; if the suggestion later becomes moot, withdraw it with dismiss_task.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "title": {"type": "string", "description": "The card heading and the new session's name: a short imperative phrase starting with a verb, under 60 characters, that makes sense on its own, e.g. \"Fix stale README badge\"."},
        "tldr": {"type": "string", "description": "One or two plain sentences on what the task would do and why it is worth doing. Shown on the card under the title, so no file paths or code."},
        "prompt": {"type": "string", "description": "The opening message of the new session: the goal, the files involved by repository-relative path, the context it needs from this conversation, and what done looks like. Markdown is fine. Hard limit 32000 characters."}
      },
      "required": ["title", "tldr", "prompt"]
    }
  },
  {
    "name": "dismiss_task",
    "title": "Withdraw a suggested task",
    "description": "Withdraw a task suggestion you queued earlier with suggest_task.\n\nCall this when a suggestion has gone stale: the issue was fixed in this session (by you or the user), it turned out not to be a problem, or you have queued a better-scoped replacement (queue the replacement first, then dismiss the old task_id).\n\nOnly a suggestion the user has not acted on can be withdrawn. If they already started or dismissed it, the result says so and nothing changes; that answer is final, so do not retry or re-flag it.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "task_id": {"type": "string", "description": "The task_id from the suggest_task result (task_1a2b3c4d)."},
        "reason": {"type": "string", "description": "Optional short note on why the suggestion is no longer needed. It does not change the outcome."}
      },
      "required": ["task_id"]
    }
  }
]
```

### 2.4 `tools/call`

`name = params.name`, `args = params.arguments` if object else `{}`.
Unknown name → JSON-RPC error `{"code": -32602, "message": "unknown tool: <name>"}` (`null` printed for a
missing name). Known tools always return a JSON-RPC **result** (tool errors are `isError: true` results):

- text result: `{"content":[{"type":"text","text":T}]}`
- error result: `{"content":[{"type":"text","text":T}],"isError":true}`

`text(v)` = `v.trim()` if string else `""`.

**`suggest_task`**

1. `prompt = text(args.prompt)`; empty → error `prompt is required and cannot be empty.`
2. UTF-16 length > 32000 → error
   `prompt is too long (<len> chars; max 32000). Trim it to the essentials; the new session can read the files itself.`
3. `title = clip200(text(args.title))`, `tldr = clip2000(text(args.tldr))` (plain truncation, no ellipsis).
4. `queued = desk.suggest(tabId, title, tldr, prompt)`; None → error
   `This session is no longer attached to a Maestria panel, so there is nowhere to show the suggestion. Mention it in your reply instead.`
5. `dropped` = `""` if no evictions, else
   `Pending suggestions are capped at 20, so the oldest ` + (`one was` | `<n> were`) + ` dropped: <list(evicted)>. `
6. Success text:
   `Noted (<task.id>). A card is showing in the Maestria panel for this session: the user can start it in a new session, in a fresh worktree or in their checkout, send it to this session, or dismiss it. If it becomes stale or superseded, call dismiss_task with this task_id. <dropped>Currently pending: <list(pending)>. Continue your current work.`
   (note: `<dropped>` is glued directly; when empty, `task_id. Currently pending:`.)

`list(tasks)` = `none` if empty, else `, `-join of `<id> "<title>"` (or just `<id>` when title is empty).

**`dismiss_task`**

1. `taskId = text(args.task_id)`; not matching `^task_[0-9a-f]{8}$` → error
   `task_id must be the id returned by suggest_task (format: task_xxxxxxxx).`
2. `out = desk.withdraw(tabId, taskId)` (`reason` is ignored).
   - withdrawn → `Task <id> withdrawn. The card is no longer shown. Continue your current work.`
   - already started → `Task <id> was already started by the user, so it is no longer pending and cannot be withdrawn. Nothing was changed.`
   - already dismissed → `Task <id> was already dismissed. Nothing was changed.`
   - unknown → `No pending task with id <id>: it was never queued from this panel, or the panel was closed. Do not re-flag it. Nothing was changed.`

### 2.5 The desk (store implementation)

- `suggest(tabId, …)`: tab must exist and be a Claude tab, else None. Create
  `SuggestedTask{id: "task_" + 8 lowercase hex from 4 secure-random bytes, title, tldr, prompt, at: now}`,
  push; while `len > 20` remove index 0, record `settled[id] = dismissed`, collect as evicted.
  `notice(tab, suggested, suggestionId: id, detail: title or None)`; save; notify.
  Returns `{task, pending: copy of list, evicted}`.
- `withdraw(tabId, taskId)`: tab or pending task missing → `(false, settled[taskId])`; else settle as `dismissed`
  → `(true, None)`.
- `settled: HashMap<id, started|dismissed>` is in-memory only (this run).
- `settle(tab, task, end)`: remove from list, record, save, notify.
- User actions: `dismissSuggestion` (settle dismissed); `runSuggestionHere(tab, task, prompt?)` (if tab exited or
  hibernated → sticky banner `<title> não está rodando — retome a conversa antes`, false; else settle started and
  submit `prompt ?? task.prompt` into the pty); `startSuggestionHere(tab, task, prompt?, label?)` (settle started,
  open a new Claude tab in the same cwd with `label ?? title-or-none` and the prompt, select it);
  worktree → new-task dialog with `prompt: task.prompt`, heading `Nova task: <title>` (when title non-empty), and
  `startedSuggestion` only if a tab was opened. Before acting, if the task is no longer in the list → banner
  `a sessão retirou essa sugestão enquanto o cartão estava aberto`.
- Tracker plugin path (`suggestionsToPlugin` persisted as top-level config `suggestionsToPlugin: true`): any
  non-dismiss choice calls `plugins.offerSuggestion(plugin, {suggestion:{id,title,tldr,prompt}, choice: <worktree|local|here>,
  origin:{tabId,cwd,root,folder,isRepo,branch}})` and records `tracking[id] = (plugin, tabId, choice)`; on
  `PluginRpcError` remove tracking + sticky banner `<plugin.name>: <message>`. When the plugin later calls
  `suggestion.created {suggestionId, ref, intro?, branch?, dir?}` (`suggestionTracked`): ignored unless tracked by
  the same plugin; if the tab/task vanished → banner `a tarefa <ref> foi criada, mas a sugestão já não estava mais no cartão`.
  Prompt = `\n\n`-join of [intro.trim() if non-empty, task.prompt, (worktree only) "Você está na branch `<branch>`. Leia o código antes de propor o plano."].
  here → runSuggestionHere; local → startSuggestionHere(label: ref); worktree → newTask(taskId: ref,
  branch: `branch ?? "feature/<ref>"`, dir: `dir ?? ref`), settle started, select.

### 2.6 Suggestion card UI (`ui/suggestions.dart`) — strings and behaviour

Floating card at the terminal's bottom-right, `inset = 12`, `maxWidth = 340`; exists only while the tab has
pending suggestions. Shows newest first (index 0 = newest); a new suggestion (count grew) unfolds the card and
resets to index 0; arrows step cyclically. Header `tarefa sugerida`; arrows (`a anterior`, `a próxima`) and
`i/n` only when n > 1; `recolher` (fold → pill `1 sugestão` / `<n> sugestões`, tooltip
`mostrar a tarefa sugerida` / `mostrar as <n> tarefas sugeridas`); `descartar`. Title (2 lines max, empty →
`tarefa sem título`), tldr (3 lines). Buttons: `detalhes` (tooltip `o cartão inteiro, com o prompt da sessão nova`)
and a split start button: front = `iniciar com worktree` (tooltip `pede o id da task, cria a worktree e abre a sessão nela`)
or, for loose folders, `iniciar localmente` (tooltip `sessão nova nesta mesma pasta`); arrow tooltip
`outras formas de iniciar` / `esconder as outras formas` expands options inside the card (grows upward):
`iniciar localmente` — `sessão nova nesta pasta, sem worktree` (only when not loose) and `fazer aqui` —
`manda o prompt pra esta sessão`. With a tracker plugin present, a switch row labelled with the plugin's
`contributes.suggestions.label`; when on, front tooltip = `cria a tarefa no <plugin.name>, a worktree dela e abre a sessão`
or (loose) `cria a tarefa no <plugin.name> e abre a sessão nesta pasta`.
Details dialog: title `tarefa sugerida por <tab.title>`, close `fechar`, body title + tldr + toggle
`ver o prompt da sessão nova` / `esconder o prompt` (prompt in mono, selectable, max 320 px tall); actions
`descartar`, `fazer aqui` (tooltip `manda o prompt pra esta sessão, como o próximo pedido`), `iniciar localmente`
(tooltip `sessão nova nesta mesma pasta, sem worktree`), `iniciar com worktree` (disabled for loose, tooltip
`avulsos não têm repositório pra tirar uma worktree`).

---

## 3. Usage and context tracking

No transcript JSONL is read. Two sources:

### 3.1 Context window — via the status line

Claude Code only sends `context_window` to the status-line command, so every session's `statusLine` runs
`$STATE/statusline.sh URL B64` which POSTs stdin to `/status/<tabId>` and then runs the user's original
status line. The script must be **byte-identical** (a test compares file content to the constant):

```sh
#!/bin/sh
# A linha de status de uma sessão aberta pela maestria: manda o que o
# Claude Code mostrou pra janela e devolve a linha de status do usuário.
url="$1"
user="$2"
input=$(cat)
# Em segundo plano e com a saída descartada: a linha de status não pode
# esperar pela janela, nem quando ela está fechada -- nem quebrar quando
# não há curl.
{ printf '%s' "$input" | curl -s -m 1 -X POST -H 'Content-Type: application/json' \
  --data-binary @- "$url"; } >/dev/null 2>&1 &
if [ -n "$user" ]; then
  # `--decode` é o que o Linux e o macOS de hoje entendem; o `-D` é a volta
  # do macOS antigo.
  cmd=$(printf '%s' "$user" | base64 --decode 2>/dev/null || printf '%s' "$user" | base64 -D)
  printf '%s' "$input" | sh -c "$cmd"
fi
```

(The file ends with a single `\n` after `fi`.)

`ContextUsage.fromStatusLine(json)` → `{percent, tokens, window}` or None:
- `cw = json.context_window` must be an object; `used_percentage` number; `context_window_size` number > 0;
  `current_usage` object. Otherwise None (before the first answer and after `/compact` they are null).
- `percent = round(used_percentage)` (Dart round: half away from zero);
  `tokens = round(input_tokens + cache_creation_input_tokens + cache_read_input_tokens)` (missing/non-number → 0;
  `output_tokens` excluded); `window = round(context_window_size)`.

`applyStatus(e)`: tab by id (none → return); `now = fromStatusLine(payload)`; if `before` and `now` are equal on
all three fields (None == None counts as equal) → return without notifying; else `tab.context = now`, notify.
Context is memory-only, never persisted.

Context chip (pane header, Claude tabs with context): 5 segments, `filled = clamp(ceil(percent/20), 0, 5)`;
color `percent >= 85` red, `>= 60` yellow, else fgDim; text `<percent>%`; tooltip
`<formatTokens(tokens)> de <formatTokens(window)> tokens`.
`formatTokens(n)`: `n < 1000` → `n`; `k = round(n/1000)`; `k < 1000` → `<k> mil`; else
`t = round(n/100000)`; `t % 10 == 0` → `<t/10> mi` else `<t/10>,<t%10> mi`.

### 3.2 Plan usage (`Usage`) — Settings › "Conta & uso"

- `GET https://api.anthropic.com/api/oauth/usage` with header `Authorization: Bearer <token>` (no other custom
  headers). 10 s connect timeout, 10 s for headers, 10 s for body.
- Token (`accessToken()`): file `$HOME/.claude/.credentials.json` → `claudeAiOauth.accessToken` (non-empty string).
  Fallback **macOS only** (and not under tests): `Sh.run("security find-generic-password -s 'Claude Code-credentials' -w")`,
  stdout parsed the same way. Never refresh the token ourselves.
- Account (`account()`): `$HOME/.claude.json` → `oauthAccount.{emailAddress, organizationName, organizationType}`;
  any failure → empty. Plan names: `claude_team` → `Claude team`, `claude_enterprise` → `Claude enterprise`,
  `claude_max` → `Claude Max`, `claude_pro` → `Claude Pro`, other → `_` replaced by space; empty → None.

Failure strings (shown verbatim):

| case | text |
|---|---|
| no token | `Não achei a credencial do Claude nesta máquina — faça login com `claude` uma vez` |
| 401 / 403 | `A credencial venceu — ela se renova sozinha da próxima vez que você usar o Claude` |
| other non-200 | `A api respondeu <status>` |
| socket error | `Sem rede pra perguntar` |
| timeout | `A api demorou demais pra responder` |
| other exception | `Não deu pra ler: <exception>` |
| body not JSON object | `A api respondeu uma coisa que não é json` |
| `limits` not array, or no usable window | `A resposta não trouxe limite nenhum` |

`parse(body)`: iterate `limits` elements that are objects; skip if `percent` is not a number;
`label = labelFor(limit)`, `percent = round(percent)` (may exceed 100), `resetsAt = parse(resets_at)` → local
time, None if missing/unparseable (accept RFC 3339 with microseconds). Order preserved. Loose top-level
`five_hour`/`seven_day` fields are ignored.
`labelFor`: kind `session` → `Sessão (5h)`; `weekly_all` → `Semanal (7 dias)`; `weekly_scoped` with
`scope.model.display_name` string → `Semanal <name>`, else `Semanal por modelo`; `""`/missing → `Limite`;
other → kind with `_` → space.
Credits: `spend` object with `enabled == true` and `used` money parseable → `Créditos extras: <used>` +
(` de <limit>` when limit parseable); else None. Money `{amount_minor, currency, exponent}`: `amount_minor`
must be a number; `exponent` default 2; value `amount_minor / 10^exponent` formatted with exactly `exponent`
decimals, `.` → `,`; result `"<currency> <amount>"` trimmed (currency default `""`).
E.g. `{1234, BRL, 2}` → `BRL 12,34`; `{900, JPY, 0}` → `JPY 900`.

UI: heading `Conta & uso`, hint `O mesmo que o /usage do Claude mostra, lido da credencial desta máquina`;
read once on open (not polled) and on the reload glyph (tooltip `Ler de novo`); account line
`<org> · <plan>` (only present parts) then email; one meter per window: label, `<percent>%`, bar width
`clamp(percent/100, 0, 1)`, color `>= 90` red, `>= 75` yellow, else accent; under it `Reseta em <shortUntil>`
when known; credits line; footer `Lida às HH:MM`. Failure → the problem text + reload glyph. Sidebar footer glyph
tooltip `Conta & uso` opens Settings on this section.
`shortUntil(at)`: past → `agora`; < 1 min → `<1min`; < 60 min → `<m>min`; < 24 h → `<h>h`; < 7 d → `<d>d`;
else `<day>/<month>` (no zero padding).

---

## 4. Notifications and OS hand-offs (`Notifier`)

`checkAlerts()` (after each hook and each `claude agents` poll):
`waiting = { "tab:<id>" → "<tab.title> · <status.callToAction>" for tabs with !done ∧ status.needsHuman }`;
for each key not yet in `alerted`: `alert("MaestrIA", text)` and add; remove keys no longer waiting;
`badge(waiting.isEmpty ? None : "<count>")`.

| function | Linux | macOS |
|---|---|---|
| `alert(title, body)` | `Sh.run("notify-send -a maestria -- " + q(title) + " " + q(body))` ⇒ argv `[$SHELL, "-lc", "notify-send -a maestria -- 'MaestrIA' '<body>'"]` | `Sh.run("osascript -e " + q("display notification " + esc(body) + " with title " + esc(title)))`; `esc(v) = "\"" + v.replace("\\","\\\\").replace("\"","\\\"") + "\""` |
| `badge(label)` | no-op (dedup by last value still applies) | native dock channel `badge` with `label ?? ""`; skipped if equal to last |
| `quickLook(path)` | file must exist; `xdg-open q(path)`; on failure `reveal` | native Quick Look; fallback `reveal` |
| `reveal(path)` | `dbus-send --session --print-reply --dest=org.freedesktop.FileManager1 /org/freedesktop/FileManager1 org.freedesktop.FileManager1.ShowItems array:string:<q("file://" + encodeFull(path))> string:""`; on failure `xdg-open q(dirname(path))` | `open -R q(path)` |
| `openLink(url)` | `xdg-open q(url)` | `open q(url)` |

All are fire-and-forget through the login shell; results only matter as `ok`. A Rust implementation may exec
`notify-send` directly with argv `["notify-send","-a","maestria",title,body]` — observable behaviour is the same
except it no longer depends on the login profile's PATH.

---

## 5. "Setup" = the Claude project-config pane (no installer)

There is **no install/check step** for Claude Code integration: hooks + MCP are injected per launch (§1.3), and
the only files Maestria writes are `$STATE/hook-port` and `$STATE/statusline.sh`. `setup.dart` +
`setup_pane.dart` implement a pane that lists and edits the files Claude Code reads in a folder.

### 5.1 Sections (`SetupSection`, display order)

| enum | label | open (accepts new files) | noun |
|---|---|---|---|
| memory | `Memória` | no | `arquivo` |
| settings | `Configuração` | no | `arquivo` |
| rules | `Regras` | yes | `regra` |
| skills | `Skills` | yes | `skill` |
| agents | `Agentes` | yes | `agente` |
| decisions | `Decisões (ADR)` | yes | `decisão` |

`about` strings (one-liners):
- memory: `O que o Claude lê no começo de toda sessão nesta pasta. Curto e prescritivo rende mais que descritivo: comandos exatos, convenções que não se deduzem do código, armadilhas conhecidas.`
- settings: `O que o harness lê, não o modelo: permissões pré-aprovadas, hooks que rodam antes ou depois de uma ação, variáveis de ambiente e servidores MCP.`
- rules: `Regras que só carregam quando um arquivo que bate no glob delas é tocado. É o que mantém o CLAUDE.md enxuto: o que só importa em `test/` fica num arquivo que só entra lá.`
- skills: `Fluxos repetíveis, invocados com /nome: gerar a release, subir o app na VM, fazer o DMG. Cada uma é uma pasta com um SKILL.md dentro.`
- agents: `Subagentes com prompt e ferramentas próprias, pra revisão com um checklist fixo ou uma busca que não precisa do contexto inteiro.`
- decisions: `Uma decisão de arquitetura por arquivo: contexto, decisão, consequências. Vale sobretudo pra decisão contra-intuitiva, a que o Claude tenderia a "corrigir" sem saber. Ele não lê esta pasta sozinho -- aponte pra ela no CLAUDE.md.`

`help` (long markdown shown in the `?` dialog) — verbatim in **Appendix A**.

### 5.2 Scan (`ClaudeSetup.scan(root)`)

Fixed slots, always listed in this order (`exists` = file exists, `fixed = true`, label = basename):

| section | rel | about |
|---|---|---|
| memory | `CLAUDE.md` | `Carregado em toda sessão e versionado com o repo. Comandos de build e teste, convenções, armadilhas, um mapa curto das pastas.` |
| memory | `CLAUDE.local.md` | `O mesmo, só seu: não vai no git. Pra caminho de máquina, preferência pessoal e anotação que não é do time.` |
| settings | `.claude/settings.json` | `Permissões, hooks e variáveis de ambiente do projeto, versionados. É o que o time inteiro compartilha.` |
| settings | `.claude/settings.local.json` | `O mesmo, só desta máquina: não vai no git. Onde as permissões que você aprovou vão parar.` |
| settings | `.mcp.json` | `Os servidores MCP do projeto, versionados -- as ferramentas de fora que toda sessão aqui ganha.` |

Then discovered (all `exists = true`, `fixed = false`):
1. rules: every file under `.claude/rules` (recursive, no symlink following) whose lowercase path ends `.md`,
   sorted by full path; `rel` relative to root; label relative to `.claude/rules`; about
   `Entra no contexto quando um arquivo que bate no `paths` do cabeçalho é lido ou editado. Sem `paths`, entra sempre.`
2. skills: each direct subdirectory of `.claude/skills` (sorted, no symlink following) containing `SKILL.md`;
   rel `.claude/skills/<d>/SKILL.md`, label `<d>`, about
   `Um fluxo com nome: o cabeçalho diz quando ele vale, o corpo diz o passo a passo. Invocado com /<d>.`
3. skills (legacy commands): `.md` files under `.claude/commands`, about
   `Um comando de barra do jeito antigo: o corpo é o prompt que /nome dispara. Skills fazem o mesmo com mais estrutura.`
4. agents: `.md` under `.claude/agents`, about
   `Um subagente: o cabeçalho diz nome, quando usá-lo e com que ferramentas; o corpo é o prompt de sistema dele.`
5. decisions: `.md` under `docs/adr`, about
   `Uma decisão: o contexto em que foi tomada, o que se decidiu e o que isso custa. Curta, e nunca reescrita -- uma decisão nova é outro ADR.`

`isJson` ⇔ rel ends `.json`.

### 5.3 Read / write / remove / new

- `read(root, rel)`: missing → `""`; else bytes decoded as UTF-8 **lossy** (never blank an editor on bad bytes).
- `write(root, rel, text)`: `mkdir -p` parent, write.
- `remove(root, rel)`: if rel starts `.claude/skills/` and ends `/SKILL.md` → delete the skill directory
  recursively (if exists); else delete the file if it exists.
- `slug(name)`: trim, lowercase, map `áàâãäéèêëíìîïóòôõöúùûüçñ` → `aaaaaeeeeiiiiooooouuuucn` char by char, replace
  runs of `[^a-z0-9]+` with `-`, strip leading/trailing `-`.
- `nextAdr(root)`: no `docs/adr` → 1; else max over direct files whose basename matches `^(\d{1,5})[-_]` + 1.
- `relFor(section, name, root)`: slug empty → None; rules `.claude/rules/<s>.md`; skills
  `.claude/skills/<s>/SKILL.md`; agents `.claude/agents/<s>.md`; decisions
  `docs/adr/<nextAdr zero-padded to 4>-<s>.md`; memory/settings → None.
- `template(rel, folderName, title="")` — `name = title` or basename with `.md` removed:
  - `CLAUDE.md`: `# <folder>\n\n## Comandos\n\n- build: \n- testes: \n- lint: \n\n## Convenções\n\n- \n\n## Armadilhas\n\n- \n\n## Mapa\n\n- `lib/` — \n\n## Decisões\n\nAs decisões de arquitetura estão em `docs/adr/`. Leia antes de mudar algo estrutural.\n`
  - `CLAUDE.local.md`: `# <folder> — só nesta máquina\n\nNão vai no git: acrescente `CLAUDE.local.md` ao `.gitignore`.\n\n- \n`
  - `.claude/settings.json` and `.claude/settings.local.json`: `{\n  "permissions": {\n    "allow": [],\n    "deny": []\n  },\n  "env": {},\n  "hooks": {}\n}\n`
  - `.mcp.json`: `{\n  "mcpServers": {}\n}\n`
  - `.claude/rules/…`: `---\npaths:\n  - "**/*"\n---\n\n# <name>\n\n- \n`
  - `.claude/skills/<skill>/…`: `---\nname: <skill>\ndescription: Quando usar esta skill, em uma frase que o Claude consiga reconhecer no pedido.\n---\n\n# <skill>\n\n## Passos\n\n1. \n`
  - `.claude/commands/…`: `# <name>\n\n`
  - `.claude/agents/…`: `---\nname: <name>\ndescription: Quando delegar pra este agente.\ntools: Read, Grep, Glob, Bash\nmodel: inherit\n---\n\nVocê é \n`
  - `docs/adr/…`: `# ADR <n>: <name>\n\n- **Status:** proposta\n- **Data:** YYYY-MM-DD\n\n## Contexto\n\n\n## Decisão\n\n\n## Consequências\n\n`
    (`<n>` = leading digits of the basename, prefix omitted if none; date = local today).
  - anything else: `""`.

### 5.4 Pane behaviour (`SetupPane`)

- One setup tab per root (`showSetup(folder, cwd?)`: reuse existing tab with same root, else create, placed to
  the right of the focused pane; focus it). Title `Claude · <basename(root)>`. Persisted as
  `setup: {root, selected?}`; `fromJson` requires non-blank root.
- Left column 232 px: per section a header (label, `?` help tooltip `O que é <label>?`, `+` tooltip
  `Nova <noun>` for open sections), slot rows (filled dot if exists, hollow+italic if not), and for an open
  section with no slots: `Criar a primeira <noun>…`.
- Editor state: `disk` (last read), `loaded` (what entered the editor: disk or template), `dirty = text != disk`,
  `edited = text != loaded`, `outside` (changed on disk while edited).
- Poll every **1500 ms**: rescan (rerender if rel/exists list changed); re-read the selected file; if different
  from `disk`: if `edited` → `outside = true`, else replace disk/loaded/text.
- Selecting a non-existent slot loads its template (save creates it). Selecting while `edited` → confirm
  dialog `Descartar o que mudou em <label>?` / `O arquivo não foi salvo. Sair dele agora perde o que você escreveu.`
  buttons `Voltar` / `Descartar`.
- Save (Ctrl+S, ⌘S on macOS; tooltip `Salvar  ctrl+S`/`Salvar  ⌘S`; enabled when dirty or not existing): write;
  error → sticky banner `Não consegui salvar <rel>: <e>`; ok → banner `<label> salvo`.
- Revert (tooltip `Voltar ao que está no disco`, enabled when edited or outside; confirm if edited).
- Add: prompt title `Nova <noun>`, field label `Nome (vira /nome)` (skills) / `Título da decisão` (decisions) /
  `Nome`; slug empty → banner `Esse nome não sobra nada depois de virar arquivo`; existing → banner
  `Já existe: <rel>` and select it; else write template (with `title = name.trim()`), error → sticky
  `Não consegui criar <rel>: <e>`; select.
- Delete (tooltip `Apagar o arquivo`): dialog `Apagar <label>?`, content `A pasta da skill vai junto, com o que mais houver dentro dela.`
  for skills else the absolute path; buttons `Cancelar` / `Apagar`; error → sticky `Não consegui apagar <rel>: <e>`;
  non-fixed slot → clear selection.
- Editor header state text: outside → `Mudou no disco enquanto você editava` (yellow); not exists →
  `Não existe ainda — salvar cria`; dirty → `Não salvo` (yellow); else `Salvo`. Other tooltips: `Abrir no vscode`,
  `Mostrar a pasta no Finder`, `Abrir a pasta no vscode`, `Fechar esta configuração`.
- Welcome (nothing selected): `O que o Claude lê em <name>`, then
  `Escolha um arquivo à esquerda pra editar. O que está apagado ainda não existe: abrir mostra um modelo, e salvar cria. O ? de cada prateleira explica o que ela é, com exemplo.`
  then each section label + about. Help dialog button `Entendi`.
- A selected rel missing from the scan is shown as a "phantom" slot (section inferred from prefix:
  rules / skills|commands / agents / docs/adr / `.json` → settings / else memory).

---

## 6. Docs / reader pane

### 6.1 Model (`MxDoc`, `DocSource`)

`DocSource`: `plan` (`plano`), `file` (`arquivo`), `message` (`recado`), `report` (`relatório`, legacy),
`plugin` (`plugin`). Only `file` is on disk (`onDisk`).
`MxDoc{source, title, text, path?, origin?, at}`; `MxDoc.file(path, origin?)` → title = basename, text `""`.
`missing` ⇔ onDisk ∧ (path None ∨ file absent). `reload()`: only onDisk with path; file absent → false; read;
equal to current text → false; else set text, `at = mtime`, true; any error → false (keep text).
`become(other)` copies all fields (a reader is a *place*; documents swap inside it).
JSON: `{source, title, path?, origin?, at ISO8601, text? (only when !onDisk, clipped to 128*1024 units + "\n\n…")}`;
`fromJson`: unknown source → None; onDisk without non-empty path → None; title default basename(path) or label.

### 6.2 Opening documents (store)

- `showDoc(doc, from?, folder?, cwd?, project?)`: reuse the reader on screen, else any reader tab; `become(doc)`,
  clear `customLabel`, place beside source if off-screen, focus, save. Else create a reader tab beside the source.
- `showPlan(tab, note?)`: none → banner `<title> não apresentou nenhum plano ainda`; title `Plano de <title>`, or
  `Plano <i>/<n> de <title>` when not the latest; `origin = tab.title`, `at = plan.at`.
- `showMessage(tab)`: blank `lastMessageFull` → banner `<title> ainda não disse nada ao terminar um turno`;
  title `Recado de <title>`.
- `showFile(path)`: missing → sticky `Esse arquivo não está mais lá: <path>`.
- `readable(path)` = `isMarkdownPath`.
- `followLink(href, from?, base?)`: blank → nothing; scheme `http|https|mailto` → `openLink`, failure → sticky
  `Não consegui abrir <href>`; else path = part before `#`; resolve (`~` expand; absolute as is; else
  `base + "/" + path`); missing → sticky `Esse link aponta pra um arquivo que não existe: <target>`; markdown →
  `showFile`; else `quickLook`. The reader first percent-decodes hrefs that do not start with a URI scheme
  (`^[A-Za-z][A-Za-z0-9+.-]*:`) and uses the doc file's directory as base (else tab cwd).

### 6.3 Rendering requirements

- Markdown flavour: GitHub (`markdown` package `ExtensionSet.gitHubWeb`): tables, task lists (`- [ ]`/`- [x]`
  rendered as checkboxes), `~~strike~~`, fenced code, extended autolinks, heading ids, footnotes, GitHub alerts,
  emoji shortcodes, inline HTML passthrough. Recommended Rust parser: `comrak` with `table, tasklist,
  strikethrough, autolink, footnotes, alerts, shortcodes, header_ids` enabled.
- Text selectable; links clickable only where the host passes a follow callback (help dialogs: inert).
- Scale derived from body size `base = 13.5` (+ tab zoom): body `base`, line-height 1.62; inline/code mono
  (`Hack`) `base - 1.7`, line-height 1.5, background `bgActive`; h1 `base+7.5`, h2 `base+3`, h3 `base+1` (fgDim),
  h4 `base` (fgDim), h5 `base-0.5` (fgDim), h6 `base-1` (fgFaint), all weight 600, height 1.3, letter-spacing -0.2;
  paddings h1 6/6, h2 top 16 bottom 4, h3 top 12 bottom 2, p bottom 2; block spacing 11; list indent 22;
  code block padding 12, radius 8, bg `bgActive`, border `border`; blockquote fgDim on `bgHover`, radius 6,
  left border 3 px accent @55%, padding 14/10/12/10; links accent, underlined (accent @45%); `del` strike +
  fgFaint; checkbox accent; tables: header/body `base-1`, header bg `bgSidebar`, all borders `border`, cell padding
  10×6, intrinsic column widths; hr = top border.
- Reader layout: centered column max width 760, padding 26/22/26/40; scroll resets to top when the document
  identity (`source|path|title`) changes; on-disk docs re-read every **1500 ms** (poll, not FS watch — editors
  that save by rename break watchers) and repaint only when `reload()` returns true.
- Empty states: missing → `Esse arquivo não está mais lá` + path; blank text → `Arquivo vazio` (onDisk) /
  `Documento vazio`.
- Header: icon per source, title, subtitle; for on-disk: `Reler do disco — ele já relê sozinho enquanto está aberto`,
  `Mostrar no Finder`, `Abrir no vscode`; always `Copiar o markdown` (banner `<title>: markdown copiado`), pin, and
  `Fechar este leitor` (closes the tab outright). Reader tabs have status `unknown` (no badge, no alerts).

---

## 7. Test cases (language-neutral)

### hook_port_test
| Given | When | Then |
|---|---|---|
| no port file | start, note port P, stop, start a new server | new server binds P; `settingsFor("tab2")` Stop hook URL port = P |
| port file holds a port occupied by another listener | start | bound port ≠ occupied; port file now = bound port |
| port file contains `nem número` | start | starts (port > 0); file rewritten with bound port |
| running server | POST `/hook/tab7` `{"hook_event_name":"Stop","session_id":"s1"}` to the port announced in settings | one event `{tabId:"tab7", name:"Stop"}` |

### hook_reducer_test (payload base `{hook_event_name, session_id:"sess-1", cwd:"/repo"}`)
| Given | When | Then |
|---|---|---|
| fresh | SessionStart | ready |
| | UserPromptSubmit `prompt:"troca o middleware  de sessão"` | working, prompts 1, lastPrompt `troca o middleware de sessão` |
| | PreToolUse Edit `file_path:/repo/lib/auth/jwt.dart` | tool, activeTool Edit, target `jwt.dart`, subtitle starts `Edit(jwt.dart)` |
| | PostToolUse Edit | working, activeTool None |
| | Stop `last_assistant_message:"Troquei os 7 call sites."` | idle, subtitle = that message |
| fresh | Notification permission_prompt | waitingPermission, needsHuman |
| fresh | PreToolUse AskUserQuestion with 2 questions (headers Receitas, Briefing) | waitingAnswer; subtitle `Pergunta: Receitas (+1)`; CTA `te faz uma pergunta`; then permission_prompt keeps waitingAnswer; PostToolUse → working, question None |
| question without header `Sobrescrevo o arquivo?` | PreToolUse | subtitle `Pergunta: Sobrescrevo o arquivo?` |
| PreToolUse Bash `rm -rf build` | permission_prompt | waitingPermission, CTA `quer aprovação` |
| fresh | Notification agent_needs_input `message:"Posso rodar os testes?"` | waitingInput, subtitle = message; then Stop → !needsHuman |
| UserPromptSubmit (`user_input`), Stop with message | Notification idle_prompt | !needsHuman, atRest, subtitle still `Pronto, os testes passam.` |
| waitingAnswer / waitingPermission | idle_prompt | status unchanged |
| PreToolUse Bash `flutter test --reporter compact -j 4 --coverage` | | target length ≤ 29, ends with `…` |
| | SessionEnd reason logout | ended |
| PreToolUse Bash | PostToolUseFailure is_interrupt false | working, activeTool None; with is_interrupt true → idle |
| working | StopFailure `error:"rate_limit"` | idle, lastMessage `rate_limit` |
| SessionStart | SessionEnd reason resume / clear | ready |
| fresh | settle() | true, ready, subtitle `Pronto` |
| after UserPromptSubmit | settle() | false, stays working |
| PostToolUse Write/Edit/MultiEdit/NotebookEdit/Read/Grep | | touched = the four writes in order (Read/Grep excluded) |
| Bash PostToolUse | `printf "# oi\n" > notas.md` / `echo linha >> docs/DIARIO.md` / `cat > 'meu plano.md' <<'EOF'…EOF` | `/repo/notas.md` / `/repo/docs/DIARIO.md` / `/repo/meu plano.md` |
| | `cat x \| tee resumo.md`, `sed -i '' 's/a/b/' LEIAME.md`, `cp modelo.md /tmp/copia.md` | `/repo/resumo.md`, `/repo/LEIAME.md`, `/tmp/copia.md` |
| | `cat > nota.md <<'MD'\n> veja o inexistente.md\nMD` | only `/repo/nota.md` |
| | `cat PLANO.md`, `grep -n TODO docs/*.md`, `mv relatorio.md docs/` | nothing |
| | `echo oi > "$OUT/nota.md"`, `sed -i "" s/a/b/ *.md`, `dart run tool/gen.dart 2>&1` | nothing |
| | `dart format lib/main.dart > /dev/null`, `python3 gera.py > saida.json` | nothing |
| Write `./saida/lista.csv`, `notas.md` | | `/repo/saida/lista.csv`, `/repo/notas.md` |
| Write a, Write b, Edit a | | `[a, b]` |
| 205 distinct writes | | len 200, first `f5`, last `f204` |
| Write `file_path:""`, no tool_input, empty payload | | touched empty |
| PreToolUse Agent (parent) | PreToolUse Grep with `agent_id:a03` | activeTool still `Agent` |
| | PostToolUse Edit with agent_id | touched includes the path |
| working after prompt `divide` | PostToolUse Read with agent_id | still working, subtitle `divide` |

### task_mcp_test
| Given | When | Then |
|---|---|---|
| | initialize with protocolVersion 2025-03-26 | echoes it; serverInfo.name `maestria`; capabilities has `tools`; instructions contain `suggest_task` |
| | `notifications/initialized` (no id) | null |
| | tools/list | names `[suggest_task, dismiss_task]`; first required `[title, tldr, prompt]` |
| desk accepts tab1 | suggest with prompt `"  Reescreva as seções 3 e 5 em MySQL.  "` | stored prompt trimmed; text contains task id and `dismiss_task` |
| | prompt `" "` | `isError: true`, nothing queued |
| | prompt of 32001 chars | `isError: true` |
| desk rejects tab9 | suggest | `isError: true` |
| queued task_00000000 | dismiss twice | first text contains `withdrawn`; second contains `already dismissed` |
| | dismiss `abc` | `isError: true` |
| | `resources/list` | error code -32601 |
| HTTP | POST `/mcp/tab1` suggest | 200, text contains `Noted`, 1 pending |
| HTTP | POST notification | 202, empty body |
| server | mcpConfigFor(tab1) / settingsFor(tab1) | type http, url `http://127.0.0.1:P/mcp/tab1`, alwaysLoad true; permissions.allow = both tool names |

### context_usage_test / statusline_test
| Given | When | Then |
|---|---|---|
| used 58.4, size 200000, usage 10/1000/115000/out 300 | fromStatusLine | 58 / 116010 / 200000 |
| used null; current_usage null; no context_window; non-object; size missing; size 0; used non-number; usage non-object; context_window non-object | | None each |
| server | POST `/hook/tab9` Stop then `/status/tab9` status | one status event (tab9, has context_window); hook events only `[Stop]` |
| store with tab t1 | applyStatus valid / with null usage / unknown tab | set (58/116010/200000) / cleared to None / ignored |
| | same valid body twice | listeners notified once |
| home, project settings, project local settings each with statusLine | userStatusLine | precedence local > project > home |
| invalid local JSON | | falls through to home |
| type `outro` | | None |
| nothing / cwd None | | None |
| no user statusLine | settingsFor | type command, no padding; argv `[sh, STATE/statusline.sh, http://127.0.0.1:P/status/t1, ""]`; ends `" ''"`; hooks include SessionStart/Stop/PreToolUse/SubagentStop; Stop URL `/hook/t1` |
| user command `echo "$HOME" \| tr a-z A-Z` padding 2 | | padding 2; argv[3] = base64(command) |
| server started | | statusline.sh content == script constant |
| user command with `'`, `"` and `$` | settings passed through `printf "%s" <Sh.q(settings)>` | identical JSON; decoded argv[3] == command |
| script with user `cat` | run with JSON stdin, closed port | stdout == JSON, exit 0 |
| empty user | | stdout empty, exit 0 |
| user `printf "%s" 'a$b' \| tr a b` | | same output as running directly (`b$b`) |
| listening server | | receives exact JSON body |
| closed port | | finishes < 1.5 s, stdout == JSON |
| PATH without curl | | stdout == JSON, stderr empty, exit 0 |
| formatTokens | 999, 1000, 1499, 1500, 116010, 116900, 999499, 999500, 1000000, 1040000, 1050000, 1200000, 1950000, 1999999, 200000 | `999, 1 mil, 1 mil, 2 mil, 116 mil, 117 mil, 999 mil, 1 mi, 1 mi, 1 mi, 1,1 mi, 1,2 mi, 2 mi, 2 mi, 200 mil` |
| chip | percent 0, 1, 20, 21, 100 | filled 0, 1, 1, 2, 5; colors 59 fgDim, 60/84 yellow, 85 red; tooltip `116 mil de 200 mil tokens` |

### usage_test
| Given | When | Then |
|---|---|---|
| sample (session 3, weekly_all 18, weekly_scoped Fable 14, spend disabled) | parse | labels `Sessão (5h)`, `Semanal (7 dias)`, `Semanal Fable`; percents 3/18/14; credits None |
| | | session resetsAt (UTC) = 2026-09-10 16:09:59.634925 |
| kind `monthly_whatever`, resets_at null | | ok, label `monthly whatever`, percent 7, resetsAt None |
| percent null + another 18 | | only `[18]` |
| percent 104 | | 104 |
| `<html>…` / `{"spend":{}}` / `{"limits":[]}` | | not ok; first problem contains `json` |
| spend enabled 1234 of 50000 BRL exp 2 | | `Créditos extras: BRL 12,34 de BRL 500,00` |
| limit null | | `Créditos extras: BRL 12,34` |
| JPY exp 0 900 | | `Créditos extras: JPY 900` |
| now 2026-09-10 13:00 | shortUntil +40min / +2h / +3d / +40s / -5min / 2026-09-30 13:00 | `40min`, `2h`, `3d`, `<1min`, `agora`, `30/9` |
| UI | settings › account | one meter per window with `N%`; `Reseta em 2h` for +2h10m; no reset text when None; `Marrow · Claude team` + email; no `·` when no account; failure text shown; reload (tooltip `Ler de novo`) re-reads once and replaces numbers; sidebar glyph `Conta & uso` opens the section, > 200 px right of `Adicionar pasta ou workspace` |

### notices_test (tabs a=`implementa` off-screen, b=`revisa` focused)
| Given | When | Then |
|---|---|---|
| | a: UserPromptSubmit, Stop | 1 notice `finished` for a, unread 1; second Stop adds nothing |
| | a asks (AskUserQuestion) while b focused; then a focused, b Bash + permission_prompt | kinds `[permission, question]` (newest first); b PostToolUse → that notice read |
| | b (focused) prompt+Stop | notice exists, unread 0 |
| a finished, list open | openNotice | focus a, unread 0, list closed |
| a finished | a UserPromptSubmit | unread 0, still 1 notice |
| a finished, then renamed | bell row | shows new name + `terminou o trabalho` |
| | status bar | `2 sessões`, count `1`, bell icon opens list |
| | toggle notices then sessions | only one list open at a time |
| a waitingAnswer | session list | sections `Esperando você  1`, `Abertas  1`; clicking a focuses it and closes list |
| b finished (focused) | | no toast; a finished → toast = that notice |
| toast | dismissToast | toast gone, notice stays unread |
| a finished | a prompts again | toasts empty |
| | do-not-disturb on | toasts cleared; later notices not toasted; unread 2 |
| | toggleNotices | toasts cleared |
| toast held | 2×toastLife | still there; released + toastLife → gone |
| toast | tap | focus a, toast gone, unread 0 |
| clear all | animation | rows exit before list empties; then `Nenhuma notificação`; clears even if the bell closes mid-animation |

### session_done_test
| Given | When | Then |
|---|---|---|
| tab | setDone(true) | done, tab still open; toJson `done: true` (absent before) |
| queued 2 steps | setDone(true) | returns 2, queue empty; second setDone(true) returns 0 |
| waitingPermission | setDone(true) | needingHuman(folder) and (project) 0; setDone(false) → 1 again |
| done | UserPromptSubmit hook | done false |
| | prompt + Stop | idle, not done |
| one done one pending | closeSettled(folder) | only pending remains |
| sidebar | done | tick icon shown, alert badges gone |

### suggestions_test
| Given | When | Then |
|---|---|---|
| tab a | store.suggest | id matches `^task_[0-9a-f]{8}$`; in a.suggestions; 1 `suggested` notice unread; `toJson.suggestions` len 1 |
| a working, PreToolUse mcp__maestria__suggest_task, suggest, PostToolUse | | suggestion notice still unread |
| | withdraw twice | first withdrawn; then `(false, dismissed)` |
| unknown tab | suggest | None |
| 20 queued | 21st | evicted = [first]; len 20 |
| notice for suggestion | suggestionOf | the task, detail = title; after dismiss → None, detail kept |
| finished notice | suggestionOf | None |
| toast | tap | text `sugeriu uma tarefa: Converter SQL`; focuses a; dialog `tarefa sugerida por implementa` with tldr |
| withdrawn before tap | tap | focuses a, no dialog |
| two suggestions | card | shows `Segunda`, `a nova`, `1/2`; `a próxima` → `Primeira`, `2/2` |
| | `descartar` | list empty, card gone |
| | `recolher` | pill `1 sugestão`; new suggestion reopens card |
| | `detalhes` | dialog + `fazer aqui` |
| | front `iniciar com worktree`; arrow reveals `iniciar localmente`, `fazer aqui`; `fazer aqui` | suggestion settled, card gone |
| | arrow | options inside card, card bottom & arrow fixed, card grows upward; `esconder as outras formas` closes |
| loose folder | | front `iniciar localmente`, arrow shows only `fazer aqui` |
| 260 px wide | | no overflow, width ≤ 236 |

### suggestion_plugins_test
| Given | When | Then |
|---|---|---|
| manifest `contributes.suggestions: {label}` | parse | label exposed, no warnings; needs `main`; non-object or blank label → FormatException; absent → None |
| shell plugin | offerSuggestion | plugin calls back `suggestion.created` with suggestionId/ref |
| end to end, choice here | | suggestion settled; pty receives `Vamos trabalhar na TASK#1` + `o pedido` |
| plugin disabled | | not a taker |
| plugin not running | startSuggestionInPlugin | suggestion stays |
| untracked id | suggestionTracked | None, suggestion stays |
| no taker | card | no switch; with taker: switch label from plugin, toggles `suggestionsToPlugin`, tooltip starts `cria a tarefa no tracker, a worktree dela`; loose → `cria a tarefa no tracker e abre a sessão nesta pasta` |

### setup_test
| Given | When | Then |
|---|---|---|
| empty repo | scan | contains CLAUDE.md, .claude/settings.json, .mcp.json; all !exists, all fixed |
| CLAUDE.md, rules/tests.md, skills/release/SKILL.md, skills/vazia/, agents/revisor.md, docs/adr/0001-flutter.md | scan | statuses/sections as expected; skill label `release`; `vazia` absent |
| | slug | `Gerar Release  do App!` → `gerar-release-do-app`; `Decisão sobre PTY` → `decisao-sobre-pty`; `***` → `` |
| | relFor | rules/skills/agents paths; memory → None; `!!!` → None |
| adr 0001, 0007, README | nextAdr / relFor decisions `usar PTY próprio` | 8 / `docs/adr/0008-usar-pty-proprio.md` |
| | write settings / remove skill SKILL.md | file written with parents / skill dir deleted with extra files |
| every section | help | contains `## O que é` and a code fence; agents help contains `subagente` and `` `tools` `` |
| every scanned slot | template | non-empty; skill has `name: release`; ADR 0003 titled `ADR 0003: PTY próprio` |
| claude tab focused | showSetup twice / other root | same tab reused, title `Claude · <base>`, 2 panes, focused; other root → 2nd setup tab |
| | MxSetup round trip / `{}` | root+selected kept / None |

### reader_test
| Given | When | Then |
|---|---|---|
| | PreToolUse ExitPlanMode `## Trocar o middleware…` | 1 plan, headline `Trocar o middleware` |
| | same plan ×3 | 1 plan |
| | `# primeiro`, `# segundo` | 2 plans, current headline `segundo` |
| tab with plan | showPlan | reader beside session (`[session, reader]`), focused |
| plan + lastMessageFull | showPlan then showMessage | same reader tab, 2 panes, source message, title `Recado de TASK#47730` |
| reader moved off screen | showPlan | reuses it, only one reader, back on screen |
| no plan | showPlan | None, 1 pane, banner contains `não apresentou nenhum plano` |
| reader | status | unknown, !needsHuman |
| | readable | `.md`, `.markdown` true; `.dart`, `.csv` false |
| plan doc | render | headings/inline code/lists rendered without markup chars |
| missing file doc | render | `Esse arquivo não está mais lá` |
| tab touched only `.dart` | DocBar | absent; with plan + `.md` → chips `Plano` and `PLANO.md` only; chip of doc open in reader highlighted; newest first |
| file doc | reload | true then text; again false; after change true |
| missing path with text | reload | false, missing, text kept |
| plan doc / file doc | toJson/fromJson | text kept / no `text` key, path kept |

---

## 8. Suggested Rust layout (each file ≤ 400 LOC)

```
crates/maestria-claude/src/
  lib.rs                 re-exports
  json.rs                clip(), utf16 helpers, sh_quote()
  hooks/server.rs        bind w/ port file, statusline write, axum router (/mcp, /status, fallback hook)
  hooks/launch.rs        settings_for(), mcp_config_for(), user_status_line(), claude command line + display
  hooks/state.rs         ClaudeStatus (+label/color/at_rest/needs_human/cta), HookState, PlanNote, subtitle, settle
  hooks/reducer.rs       apply()
  hooks/touched.rs       touch(), shell_markdown(), heredoc stripping, absolute(), is_markdown_path()
  mcp/protocol.rs        handle(), one(), envelopes, HTTP adapter (200/202/405)
  mcp/tools.rs           const INSTRUCTIONS, descriptions, tools_json(), suggest/dismiss handlers
  mcp/desk.rs            trait TaskDesk, SuggestionQueued, SuggestionEnd, NoDesk
  suggestions.rs         store-side queue: suggest/withdraw/settle/evict/task id, tracker plumbing
  notices.rs             MxNotice, notice(), read_notices_of(), toast caps/timers
  context.rs             ContextUsage::from_status_line, format_tokens, chip thresholds
  usage/fetch.rs         token lookup (file / macOS keychain), reqwest GET with timeouts, error strings
  usage/parse.rs         parse(), label_for(), credits/money, account(), plan names, short_until
  notify.rs              alert/badge/quick_look/reveal/open_link
  setup/scan.rs          SetupSection, SetupSlot, scan(), read/write/remove
  setup/templates.rs     slug, next_adr, rel_for, template()
  setup/help/*.md        Appendix A texts via include_str!
  docs.rs                DocSource, MxDoc (reload/json), follow_link resolution
  statusline.sh          include_str! (byte-identical script)
```

Crates: `tokio` (rt-multi-thread, net, fs, time, process), `axum` 0.7 (or bare `hyper` 1.x — routing is trivial),
`serde` + `serde_json` (`preserve_order`), `base64`, `regex` (ASCII classes), `rand`/`getrandom` (task ids),
`reqwest` (rustls, timeouts) for the usage call, `chrono` (ISO 8601, local time, `resets_at`), `comrak`
(GFM → AST for whatever UI toolkit renders it), `percent-encoding` (link decode, `file://` URI), `thiserror`.
**Do not use `rmcp`**: it adds SSE / session-id semantics and its own error mapping; the wire format here
(version echo, 202 for notifications, `-32700/-32600/-32601/-32602`, tool errors as `isError` results) is
~150 LOC to hand-roll and must match byte-for-byte.

---

## Appendix A — `SetupSection.help` markdown (verbatim)

Each block below is the exact string (Dart `\$` escapes already resolved to `$`). Store them as files and
`include_str!` them.

### help: `memory`

`````markdown
## O que é

O `CLAUDE.md` é o texto que o Claude Code lê **no começo de toda sessão** aberta nesta pasta, antes da sua primeira mensagem. É o jeito de dizer uma vez o que você repetiria em toda conversa.

Ele fica na raiz do repo e vai no git: é do time. O `CLAUDE.md` da sua pasta pessoal (`~/.claude/CLAUDE.md`) vale pra todos os projetos; este vale só aqui, e os dois são lidos juntos. Também dá pra ter um `CLAUDE.md` dentro de uma subpasta, que só entra quando o Claude mexe em arquivos de lá.

O `CLAUDE.local.md` é o irmão que **não vai no git**: caminho de máquina, preferência sua, anotação que não é do time. Acrescente-o ao `.gitignore`.

## O que colocar

Curto e prescritivo rende mais que descritivo. O que vale:

- **Comandos exatos** de build, teste e lint. "rode `flutter test`" evita que ele descubra sozinho a cada vez.
- **Convenções** que não se deduzem do código: padrão de commit, de branch, de nome.
- **Armadilhas**: "não rode X, use Y", "o módulo Z depende de W".
- **Um mapa curto** das pastas: cinco a dez linhas sobre onde mora o quê.
- **Um ponteiro** pras decisões em `docs/adr/`, se você as escreve.

O que não vale: despejar a arquitetura inteira. Passando de umas duzentas linhas o sinal se perde no ruído, e o que importa deixa de ser lido.

## Puxar outro arquivo: o `@`

Uma linha com `@caminho` dentro do `CLAUDE.md` traz o conteúdo daquele arquivo pro contexto, como se estivesse colado ali. É o jeito de o `CLAUDE.md` ficar curto e ainda assim carregar o que importa:

```markdown
Convenções de commit: @docs/commits.md
Decisões de arquitetura: @docs/adr/README.md
```

O caminho é relativo ao `CLAUDE.md`, e vale `~/` pra pasta pessoal. Importação dentro de importação também funciona, até uns cinco níveis.

## Exemplo

```markdown
# maestria

## Comandos
- testes: /caminho/do/fvm/flutter test
- analyze: /caminho/do/fvm/flutter analyze

## Convenções
- commit: `TASK#123 add ...`, uma linha, sem corpo

## Armadilhas
- `flutter` no PATH é alias do zsh; use o binário do fvm
```
`````

### help: `settings`

`````markdown
## O que é

O `.claude/settings.json` é lido pelo **harness** do Claude Code, não pelo modelo. Onde o `CLAUDE.md` é uma instrução que o Claude *pode* seguir, isto é uma regra que a ferramenta *faz cumprir*. Vai no git.

O `settings.local.json` é o mesmo formato, só desta máquina, e o Claude Code já o põe no `.gitignore`. É pra onde vão as permissões que você aprova clicando em "sempre permitir".

## O que dá pra configurar

- **Permissões**: `allow` e `deny` de comandos e ferramentas. `"Bash(flutter test:*)"` deixa rodar testes sem perguntar. `"Read(./.env)"` em `deny` impede de ler o arquivo.
- **Hooks**: scripts que rodam antes ou depois de uma ação, por exemplo `dart format` depois de toda edição, ou um lint que bloqueia o commit se falhar. Os eventos são `PreToolUse`, `PostToolUse`, `Stop`, `Notification` e outros.
- **Variáveis de ambiente** em `env`, pra toda sessão desta pasta.
- **Modelo padrão** e outras opções do harness.

## Exemplo

```json
{
  "permissions": {
    "allow": ["Bash(flutter test:*)", "Bash(flutter analyze:*)"],
    "deny": ["Read(./.env)"]
  },
  "hooks": {
    "PostToolUse": [{
      "matcher": "Edit|Write",
      "hooks": [{ "type": "command", "command": "dart format $CLAUDE_FILE_PATHS" }]
    }]
  }
}
```

## O .mcp.json: MCP

**MCP** é o *Model Context Protocol*: um padrão pra dar ao Claude ferramentas de fora, e contexto de fora. Um servidor MCP é um programa pequeno que expõe ferramentas (consultar um banco, abrir uma task no tracker, ler um Figma), e o Claude passa a poder chamá-las como chama `Read` ou `Bash`.

O `.mcp.json` na raiz lista os servidores **deste projeto**, e vai no git: quem clona ganha as mesmas ferramentas, e cada pessoa aprova na primeira sessão. Os seus servidores pessoais ficam em `~/.claude.json`, fora daqui.

```json
{
  "mcpServers": {
    "postgres": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-postgres", "postgresql://localhost/app"]
    },
    "wiboor": {
      "type": "http",
      "url": "https://api.wiboor.com/mcp",
      "headers": { "Authorization": "Bearer ${WIBOOR_TOKEN}" }
    }
  }
}
```

Um servidor pode ser um comando local (`command` + `args`) ou um endereço (`type: http` ou `sse`). `${VAR}` puxa da variável de ambiente, pra o token não ir pro git.
`````

### help: `rules`

`````markdown
## O que é

Uma regra é um pedaço de `CLAUDE.md` que **só entra no contexto quando é preciso**. Cada `.md` em `.claude/rules/` pode dizer, no cabeçalho, a quais arquivos ele se aplica; quando o Claude lê ou edita um arquivo que bate no glob, a regra é carregada. Sem `paths`, ela entra sempre, como se fosse parte do `CLAUDE.md`.

É o que mantém o `CLAUDE.md` enxuto: o que só importa em `test/` fica num arquivo que só aparece quando se está em `test/`.

## O frontmatter

O bloco entre os dois `---` no topo do arquivo é o **frontmatter**: um cabeçalho em YAML com dados *sobre* o arquivo, que o Claude Code lê antes do texto. Aqui ele tem um campo só:

- `paths`: lista de globs. `test/**` é tudo dentro de `test/`; `lib/ui/*.dart` é só o primeiro nível de `lib/ui/`. A regra entra quando um arquivo que bate em algum deles é lido ou editado.

Sem frontmatter, a regra vale sempre.

## Quando usar

- Convenções de uma parte só do repo: como escrever um teste, como é um widget aqui, o que um migration precisa ter.
- Um `CLAUDE.md` que passou de duzentas linhas e quer ser repartido.
- Instruções que se organizam em subpastas, que o Claude Code lê do mesmo jeito.

## Exemplo

`.claude/rules/tests.md`:

```markdown
---
paths:
  - "test/**"
---

# Testes

- Um `group` por comportamento, nomes em português, na primeira pessoa do que o código faz.
- Nada de `pumpAndSettle` com animação infinita: use `pump` com duração.
- Store sob teste sempre com `addTearDown(store.dispose)`.
```
`````

### help: `skills`

`````markdown
## O que é

Uma skill é um **fluxo com nome**, que você invoca digitando `/nome` na sessão, ou que o Claude escolhe sozinho quando o seu pedido bate na descrição dela. Cada uma é uma pasta em `.claude/skills/` com um `SKILL.md` dentro; o cabeçalho diz o nome e quando ela vale, o corpo diz o passo a passo.

A diferença pra um parágrafo no `CLAUDE.md`: a skill só é lida quando chamada, então pode ser longa e detalhada sem custar contexto nas outras conversas. E pode levar arquivos junto na mesma pasta: um script, um modelo, uma referência.

## O frontmatter

O bloco entre os dois `---` no topo do `SKILL.md` é o **frontmatter**: um cabeçalho em YAML que o Claude Code lê pra saber o que a skill é *sem* ler o corpo dela. É por ele que a skill aparece no `/` e é escolhida sozinha.

- `name`: o nome depois da barra. Minúsculas e hífens.
- `description`: **quando** usar. É o texto que o Claude compara com o seu pedido pra decidir invocar a skill por conta própria; escreva os gatilhos ("use quando o pedido for fechar versão, gerar DMG ou publicar").
- `allowed-tools` (opcional): as ferramentas que a skill pode usar sem perguntar, como `Bash(git *)`.
- `disable-model-invocation` (opcional): `true` deixa a skill só pro `/nome` digitado, sem o Claude escolhê-la sozinho. Pra o que não pode rodar sem você pedir.

## Quando usar

Tudo que você faz repetidas vezes e sempre do mesmo jeito:

- gerar a release: bump de versão, changelog, tag, DMG;
- subir o app numa VM pra reproduzir um bug;
- fazer a revisão de um PR com o checklist do time;
- montar um relatório com formato fixo.

## Exemplo

`.claude/skills/release/SKILL.md`:

```markdown
---
name: release
description: Gera uma release do maestria. Use quando o pedido for fechar versão, gerar DMG ou publicar.
---

# Release

1. Confirme que `flutter analyze` e `flutter test` passam.
2. Suba a versão em `pubspec.yaml` e peça o número novo se não foi dito.
3. Rode `packaging/macos/make-dmg.sh`.
4. Commit em uma linha, no padrão do time, e tag `vX.Y.Z`.
```

Os `.md` soltos em `.claude/commands/` são o formato antigo da mesma ideia: o corpo é o prompt que `/nome` dispara. Ainda funcionam; skills fazem o mesmo com mais estrutura.
`````

### help: `agents`

`````markdown
## O que é

Um agente (ou **subagente**) é um segundo Claude que a sessão principal chama pra fazer uma tarefa e voltar com o resultado. Ele roda **com o próprio contexto**, o próprio prompt de sistema e a própria lista de ferramentas; a conversa principal não vê o que ele leu, só o que ele respondeu.

Isso serve pra duas coisas:

- **Não sujar o contexto.** Uma busca que abre trinta arquivos pra achar um deles enche a conversa principal de código que não vai ser usado. Feita por um agente, só a resposta volta.
- **Especializar.** Um revisor que só pode ler, com um checklist fixo, revisa melhor que a mesma sessão que acabou de escrever o código.

O Claude Code já vem com alguns (o explorador, o planejador). Os seus ficam em `.claude/agents/`, um `.md` por agente, e a sessão principal os usa quando o pedido bate na descrição, ou quando você pede pelo nome.

## O frontmatter

O bloco entre os dois `---` no topo é o **frontmatter**: o cabeçalho em YAML com os dados do agente, que a sessão principal lê pra decidir quando e como chamá-lo.

- `name`: como ele se chama.
- `description`: **quando** delegar pra ele. É o que a sessão principal lê pra decidir usá-lo; escreva como uma frase de gatilho.
- `tools`: o que ele pode usar. Um revisor sem `Edit` e sem `Write` não consegue "consertar" o que devia só apontar.
- `model`: `inherit` usa o da sessão; `haiku` deixa uma busca simples mais barata.

O corpo é o prompt de sistema dele.

## Exemplo

`.claude/agents/revisor.md`:

```markdown
---
name: revisor
description: Revisa um diff ou PR procurando bug, quebra de convenção e teste faltando. Use ao pedir revisão, code review ou "olha isso antes de eu commitar".
tools: Read, Grep, Glob, Bash
model: inherit
---

Você revisa código do maestria. Não edita nada: aponta.

Pra cada achado diga arquivo e linha, o problema em uma frase, e como reproduzir se for bug. Ordene do mais grave pro mais leve. Cheque: null safety, timers sem cancel, setState depois de dispose, e se toda mudança de comportamento tem teste.
```
`````

### help: `decisions`

`````markdown
## O que é

Um ADR (*Architecture Decision Record*) é **uma decisão de arquitetura por arquivo**: o contexto em que foi tomada, o que se decidiu, e o que isso custa. Fica em `docs/adr/`, numerado, e nunca é reescrito: uma decisão que muda vira um ADR novo que substitui o antigo.

Não é um formato do Claude Code, é um formato de time. Mas ajuda o Claude por um motivo específico: a decisão **contra-intuitiva**. "Por que o terminal tem um VT próprio em vez de usar a lib X?" é exatamente o tipo de coisa que uma sessão nova tenderia a "corrigir" sem saber. Com o ADR, ela lê o porquê antes.

## O detalhe que importa

O Claude **não lê esta pasta sozinho**. É o `CLAUDE.md` que tem de apontar pra ela: uma linha como "as decisões de arquitetura estão em `docs/adr/`; leia antes de mudar algo estrutural". Sem isso, os ADRs são documentação pra gente, e só.

## Exemplo

`docs/adr/0003-vt-proprio.md`:

```markdown
# ADR 0003: parser VT próprio em vez do do xterm

- **Status:** aceita
- **Data:** 2026-03-10

## Contexto
O xterm.dart perde a cor de fundo em sequências OSC compostas, e o
Claude Code as usa no cabeçalho.

## Decisão
Um parser próprio em `services/vt.dart`, só pro subconjunto que o
Claude Code emite.

## Consequências
Menos dependência, mais código nosso. Toda sequência nova precisa de
um teste em `vt_test.dart`.
```
`````
