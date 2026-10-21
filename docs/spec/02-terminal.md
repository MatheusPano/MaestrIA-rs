# 02 — Terminal, PTY & Input

Behavioral spec of Maestria's terminal subsystem, written so a Rust implementer
who never reads the Dart code can rebuild it with identical behavior.

Source files (Flutter app, read-only reference):
`lib/services/pty.dart`, `lib/services/vt.dart`, `lib/services/links.dart`,
`lib/services/shortcuts.dart`, `lib/services/dictation.dart`,
`lib/services/layout.dart`, `lib/services/shell.dart` (spawn env),
`lib/ui/terminal_pane.dart`, `lib/ui/panes.dart`, `lib/ui/keys.dart`,
`lib/ui/result_strip.dart`, `lib/ui/panel.dart` (pane key hint / card),
`lib/ui/dialogs.dart` (`showTerminalMenu`), `lib/theme.dart` (pty font/theme),
and the call sites in `lib/services/store.dart`.

Packages: `xterm ^4.0.0` (VT emulator + Flutter view) and `flutter_pty ^0.4.2`.

UI strings are Portuguese and are quoted verbatim; keep them byte-identical.

---

## 0. Glossary

| Term | Meaning |
|---|---|
| **tab / session / painel** (`MxTab`) | One entry in the sidebar. Kinds: `claude`, `shell`, `reader`, `setup`, `plugin`. Only `claude` and `shell` own a pty (`TermSession`). |
| **pane** | A rectangle on screen showing one tab. Panes form a tree (§2.1). A pane is a *position*, not a session: the same pane widget can be handed another tab. |
| **launcher** | A saved program (`btop`, `npm run dev`) — a `shell` tab started with a command. |
| **hibernated** | A claude tab whose process was killed on purpose; conversation resumable via `--resume`. |
| **focused pane** | `store.focusedPaneId`; the pane receiving keyboard input. |
| **alt screen** | DEC alternate screen buffer (`CSI ? 1049 h/l`). Claude Code runs its whole TUI in it. |

---

## 1. Sessions: spawn, output, exit, status

### 1.1 The login shell (`Sh.shell`)

Resolved once at startup:

1. macOS: always `/bin/zsh`.
2. Otherwise `$SHELL` if set, non-empty and the file exists.
3. Else the first existing of `/bin/bash`, `/usr/bin/bash`, `/bin/zsh`, `/usr/bin/zsh`.
4. Else `/bin/sh`.

Shell quoting helper `Sh.q(v)` = `'` + v with every `'` replaced by `'\''` + `'`.

### 1.2 Environment of every child (`Sh.env`)

`Sh.env` = the app's full `Platform.environment`, with `PATH` replaced by
`pathWithFallbacks(inheritedPATH)`:

- Split inherited PATH on `:`, drop empty entries.
- Append (in this order, only if the directory exists and is not already present):
  `$HOME/.local/bin`, `$HOME/.claude/local`, `$HOME/.bun/bin`, `$HOME/.volta/bin`,
  `$HOME/.npm-global/bin`, `$HOME/.pub-cache/bin`, `$HOME/.cargo/bin`
  (these only when HOME is non-empty), then `/opt/homebrew/bin`, `/opt/homebrew/sbin`,
  `/usr/local/bin`, `/usr/local/sbin`, and on Linux `/home/linuxbrew/.linuxbrew/bin`.
- Join with `:`.

Pty env = `{...Sh.env, ...capabilities, ...callerEnv}` (later wins) where
`capabilities = {'FORCE_HYPERLINK': '1'}`.

`FORCE_HYPERLINK=1` makes `claude` (supports-hyperlinks family) emit real OSC 8
links instead of `label (url)`. It also neutralises a misleading inherited
`VTE_VERSION` on Linux.

**TERM.** The Dart code never sets TERM. `flutter_pty` (0.4.x, from memory of its
source — verify) builds the env as `TERM=xterm-256color`, `LANG=en_US.UTF-8`, a few
copied vars, then overlays the caller map. Since the caller map is the *entire*
app environment, an app launched from a terminal passes that terminal's `TERM`
through; launched from Finder/desktop there is no TERM and `xterm-256color` wins.
**Rust recommendation:** set `TERM=xterm-256color` explicitly (and `COLORTERM=truecolor`
is safe since the renderer supports 24-bit), and `LANG` only if absent. This is
the one intentional deviation worth making; document it.

### 1.3 argv

| Call | argv (executable = `Sh.shell`) |
|---|---|
| `startShell(cwd)` | `[-l]` |
| `startCommand(cmd, cwd, env?, display?)` | `[-lc, "exec " + cmd]` |

`exec` makes the pty child pid *be* the target program's pid (needed to match a
tab to a row of `claude agents --json` by pid — `applyAgents` maps `term.pid` → tab).

Before `startCommand` spawns, it writes a dim "note" line into the terminal
buffer: `"\x1b[2m$ " + (display ?? command) + "\x1b[0m\r\n"`.

Other buffer-only lines:
- `remark(text)` → `"\x1b[2m[" + text + "]\x1b[0m\r\n"`.
- Missing cwd → `"\x1b[31ma pasta $cwd não existe — nada foi iniciado.\x1b[0m\r\n"`.
- On exit → `"\r\n\x1b[2m[processo saiu com $code]\x1b[0m\r\n"`.

#### Claude launch command (`_launchClaude`)

Parts joined by a single space (`name` = `tab.agentName ?? basename(cwd)`; agentName
defaults to the custom label if non-empty else basename(cwd)):

```
claude
--mcp-config <q(hooks.mcpConfigFor(tabId))>        # first: it is variadic
--name <q(name)>                                    # terminates --mcp-config
--settings <q(hooks.settingsFor(tabId, cwd))>
[--append-system-prompt <q(brief)>]                 # if project brief non-empty (trimmed)
[--resume <q(resumeId)>]                            # if resuming
[<q(openingPrompt)>]                                # positional, last; if non-empty (trimmed)
```

The `display` (what the dim note shows, hiding JSON blobs):
```
claude --mcp-config <tarefas :PORT> --name 'NAME' --settings <hooks :PORT>[ --append-system-prompt <briefing de PROJECTNAME>][ --resume …][ <prompt inicial>]
```

3 s after launch, if the tab still exists and has not exited, `hooks.settle()` runs
(marks the prompt ready if nothing else spoke first; see hooks spec).

#### Shell / launcher

`openShell(folder, cwd?, command?, launcher?)`: `run = command ?? launcher?.command`;
null → `startShell(cwd)`, else `startCommand(run, cwd)`. `cwd` defaults to `folder.root`.

### 1.4 Sizes

- Terminal object is created with xterm defaults: **80 cols × 24 rows**, scrollback
  `maxLines = 8000` (`TermSession({int scrollback = 8000})`).
- The pty is opened with `columns = terminal.viewWidth`, `rows = terminal.viewHeight`
  (i.e. the current grid size, 80×24 until the view first lays out).
- Layout: the view auto-fits cols/rows to its box (inner padding **L12 T10 R12 B10** px)
  and calls `terminal.resize(cols, rows, pixelW, pixelH)`; `onResize` forwards
  `pty.resize(rows, cols)`. Pixel sizes are not forwarded to the pty.
- Reflow on resize is on (xterm default `reflowEnabled: true`).

### 1.5 Output path

```
pty bytes → UTF-8 decode (malformed → U+FFFD, never drop the chunk)
          → VtScrubber (stateful, per session)
          → VtTerminal.write
```

`allowMalformed` matters: one invalid byte used to drop the whole chunk, losing an
`ESC[?1049l` and leaving the pane stuck in the alt screen. Note the decoder is a
streaming decoder: a multi-byte sequence split across reads is reassembled.

### 1.6 Input path

`terminal.onOutput = data → pty.write(utf8(data))`. Everything the emulator emits
(keys, paste, mouse reports, DA replies) goes through `onOutput`.

### 1.7 Exit

- If cwd does not exist (checked before spawning): `exited=true`, `exitCode=-1`, red
  line written, `onExit` fired on the next microtask (the caller wires `onExit`
  right after calling start). No spawn.
- On process exit: if this pty is no longer the session's current pty (a previous
  life killed late), ignore. Else `exited=true`, `exitCode=code`, write the
  "processo saiu" line, fire `onExit`.
- `relaunch(command, cwd)`: only if `exited`; resets `exited/exitCode`, writes `\r\n`,
  then `startCommand(command, cwd)` (scrollback kept).
- `park()`: only before the first start (`_pty == null`): `exited=true`, `exitCode=null`.
  Used for tabs born hibernated.
- Every `_start` first resets `exited=false, exitCode=null`.

Store's `onExit` handler (registered in `_register`):
1. If the tab was removed from `tabs` already → do nothing.
2. If kind==claude and alive < 5 s since `startedAt` → sticky banner
   `earlyExitMessage(title, code)`:
   - code 127: `"$title: não achei o \`claude\` no PATH do app — instale-o, ou ponha a pasta do binário no ${profileFile} (o rc do shell interativo o app não lê)"` with `profileFile` = `~/.zprofile` (macOS) / `~/.profile` (other).
   - else: `"$title: o Claude saiu na largada (código ${code ?? '?'}) — abra o painel pra ver o motivo"`.
3. Emit plugin event `session.exited {tabId, code}` (activation `onSession`).
4. Save config, notify.

### 1.8 Kill / hang-up

`kill()` (async):
1. If no pty or already exited → return.
2. Send **SIGHUP to the process group** `-pid`; if that fails, to `pid`. If both fail
   → return. (Never signal when `pid <= 1`.)
3. Wait for exit **or 3 s** (`_grace`).
4. If still not exited → SIGKILL (group, then pid).

Rationale (must be preserved): interactive zsh ignores SIGTERM; the pty child is a
session leader (`setsid`) so `-pid` reaches MCP servers, `npm run dev`, pagers.

`hangUp()`: SIGHUP only, no wait (app teardown path, `dispose`).

Call sites: `closeTab` (unawaited `kill`), `hibernate` (`kill` or `park` if no pid),
`wake` (awaits `kill` if not exited, then writes `\r\n`, relaunches claude with
`--resume`), `shutdown` (awaits `kill` of all tabs in parallel, after saving config),
`dispose` (`hangUp` all).

### 1.9 Writing into a session from the app

| API | Behavior |
|---|---|
| `newline()` | `send("\x1b\r")` — ESC+CR, what Claude reads as a newline in the prompt. Bound to Shift+Enter (§3.3). |
| `submit(text)` | If exited or `text.trim()` empty → no-op. `paste(text)` (bracketed if the program enabled `?2004`), wait **250 ms**, if still alive send Enter key (`\r`). Used by follow-up flows, plugin `terminal.sendText {submit:true}`, task prompts, dictation-with-submit. |
| `pasteClipboard({imagesViaCtrlV})` | Read clipboard text; if non-empty → `paste(text)` and clear selection. Else if `imagesViaCtrlV` (true iff tab kind is claude) → `send("\x16")` (^V; Claude reads the image off the system clipboard itself). Else nothing. |
| `copySelection()` | If a selection exists → clipboard = `selectedText(terminal, selection)` (§2.6). No selection = silent no-op. |
| `terminal.paste(text)` | Plugin `terminal.sendText` without submit. |
| `wheel(up, col, row)` | §2.4. |

`VtTerminal.send(data)` = "app-originated input": clears the echo guard (`_lastInput = null`)
then feeds the data as text input (no dedupe). `paste` also clears `_lastInput`.

### 1.10 Status inference — none from terminal bytes

The terminal output is **not parsed for status**. Session status:

- `reader/setup/plugin` → `unknown`.
- `shell` → `ended` if exited, else `unknown`.
- `claude` → `hooks.status`, driven exclusively by Claude Code HTTP hooks (see the
  hooks spec). The terminal contributes only the exit event (§1.7) and the
  `claude agents --json` pid match.

Context usage (header chip) comes from Claude Code's **statusLine** JSON, relayed by a
script — §1.11.

### 1.11 Status line relay & context usage (statusline_test, context_usage_test)

`--settings` JSON contains:
```json
"statusLine": {"type":"command",
  "command":"sh '<stateDir>/statusline.sh' 'http://127.0.0.1:PORT/status/TABID' '<base64(userCmd) or empty>'",
  "padding": <copied from user statusLine if present>}
```
User's own statusLine is looked up (first match wins, skipping missing/invalid JSON,
requires `type=="command"` and string `command`): `$cwd/.claude/settings.local.json`,
`$cwd/.claude/settings.json`, `$HOME/.claude/settings.json`.

Script (written verbatim on every server start to `$mxStateDir/statusline.sh`):
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

`POST /status/<tabId>` → `StatusEvent(tabId, payload)` (separate stream from hooks;
response always `200 {}`).

`ContextUsage.fromStatusLine(json)`: null unless `json.context_window` is a map with
`used_percentage` numeric, `context_window_size` numeric > 0, `current_usage` a map.
`percent = round(used_percentage)`, `tokens = round(input_tokens + cache_creation_input_tokens + cache_read_input_tokens)` (missing/non-numeric keys = 0; `output_tokens` excluded),
`window = round(context_window_size)`.

`applyStatus`: unknown tab → ignore; set `tab.context = parsed` (null clears); notify
only if (percent, tokens, window) changed. Never persisted.

---

## 2. Emulator fixes, panes, mouse, selection, links

### 2.1 VT corrections over the base emulator (`vt.dart`)

xterm 4.0 had six bugs; Maestria fixes them. **Any Rust VT engine must reproduce the
corrected behavior** (alacritty_terminal already does most natively — see §6).

| # | Bug | Required behavior |
|---|---|---|
| 1 | `SGR 22` only cleared faint | `CSI 22 m` clears **both bold and faint**. |
| 2 | `CSI > 4;2 m` (XTMODKEYS) parsed as SGR → underline+faint | Any CSI with a private prefix (`<`, `=`, `>`, bytes 0x3C–0x3E right after `CSI`) whose final byte is `m` is **dropped**. Other private CSI (e.g. `CSI > c`, `CSI < u`) pass through untouched. |
| 3 | `CSI ? 1049 l` didn't restore attributes | Leaving the alt screen **resets the cursor style/attributes** (not the saved position: `resetCursorStyle()` after switching to the main buffer). |
| 4 | ibus delivers one composed char 3× | Text-input dedupe: §3.5. |
| 5 | Copy dropped never-written cells | Selection-to-text treats null cells as spaces: §2.6. |
| 6 | `CSI 3 J` (and alt-screen resize trimming) left line indices stale → selection/links offset by N rows | After `eraseScrollbackOnly` and after every `resize`, re-seat every buffer line's index. Any anchor (selection end, OSC 8 span end) must refer to the correct row afterwards. |

#### VtScrubber — exact algorithm (stateful per session)

Constant `_maxFragment = 32`.

```
call(chunk):
  input = held + chunk ; held = ""
  out = ""; cursor = 0
  loop while cursor < len(input):
    csi = indexOf("\x1b[", from cursor)
    if csi < 0:
      esc = len-1 if input endsWith "\x1b" else len
      out += input[cursor:esc]; held = input[esc:]; break
    out += input[cursor:csi]
    if csi+2 >= len: hold(csi); break               # prefix byte not here yet
    if input[csi+2] not in 0x3C..0x3E:
      out += "\x1b["; cursor = csi+2; continue
    end = csi+3
    while end < len and input[end] not in 0x40..0x7E: end++
    if end == len: hold(csi); break
    if input[end] != 'm': out += input[csi:end+1]
    cursor = end+1
  return out

hold(start): frag = input[start:]
  if len(frag) > 32: out += frag   # give up, emit as-is
  else: held = frag
```

Note: only the *last* char matters for the lone-ESC rule; an unterminated non-private
CSI is emitted immediately (only private ones are held).

### 2.2 Pane tree (`layout.dart`) — pure model

```
PaneNode = PaneLeaf{tabId (mutable)} | PaneSplit{axis: row|column, children[], weights[] (sum 1)}
DropSide = left|right|top|bottom|center
  axis: left/right → row ; top/bottom → column
  leading: left/top
Panes.minWidth = 240, Panes.minHeight = 140 ; minFor(row)=240, minFor(column)=140
```

Operations (all by tab id):

- `order(root)`: DFS leaf ids, left-to-right/top-to-bottom. `count`, `has`.
- `swap(root, targetId, tabId)`: leaf showing target now shows tabId.
- `insert(root, tabId, targetId, side)`:
  - root is a leaf: if not target → unchanged; else new split(side.axis, leading? [new, root] : [root, new], [0.5,0.5]).
  - else recursive: find the leaf `targetId` at index i in its parent split:
    - parent.axis == side.axis → insert new leaf at `leading ? i : i+1`; target weight halves, new leaf gets the other half (siblings untouched).
    - else replace child i with split(side.axis, [new,child] or [child,new], [0.5,0.5]).
- `remove(root, tabId)` → new root or null:
  - removed leaf: its weight is redistributed to the remaining siblings **proportionally** to their existing weights (if their sum ≤ 0 → equal split).
  - a child that collapsed into a split of the same axis as its parent is **absorbed** (flattened) with weights `parentShare * childWeight[k]`.
  - a split left with one child returns that child.
- `edge(root, tabId, side)`: if root is a split along side.axis → every weight × n/(n+1), insert new leaf at 0 or n with weight 1/(n+1); else wrap: split(side.axis, [new,root]/[root,new], [0.5,0.5]).
- `resize(split, gutter, deltaPx, totalPx)`: no-op if gutter invalid or total ≤ 0.
  `floor = clamp(minFor(axis)/total, 0, 0.45)`, `a,b = weights[g], weights[g+1]`,
  `lower = floor - a`, `upper = b - floor`; if lower > upper no-op;
  `moved = clamp(delta/total, lower, upper)`; if 0 no-op; `a+=moved, b-=moved`.
- `copy`, `sameShape(a,b)` (same axes, same leaf ids in same positions, weights ignored).
- Persistence `toJson(node, indexOf)`: leaf → int index into the saved panes list (or
  dropped if −1); split → `{"axis":"row"|"column","weights":[…normalized],"children":[…]}`;
  splits with 0 kept children → null, with 1 → that child. `fromJson(json, tabIdAt)`
  mirrors it (missing weights default 1, then normalize; unknown axis → row).
  Legacy layout keys `active`/`split` (two slots) → leaf or row of two at 0.5/0.5.

### 2.3 Pane UI (`panes.dart`, `terminal_pane.dart`)

**Rendering the tree.** Splits render children separated by a grip exactly `Mx.gap = 8`
px wide/tall; available length = `box - 8*(n-1)` distributed by
`flex = max(1, round(weight*10000))`. A leaf whose tab no longer exists renders empty.
Each leaf renders by kind: reader → DocPane, plugin → PluginPane, setup → SetupPane,
else TerminalPane. `focused = (store.focusedTab.id == tab.id)`;
`showFocus = paneCount > 1`.

**Grip** (`_Grip`): cursor resize-left-right (row) / up-down (column); visible bar
3 px thick, 46 px long when hovered/dragging else 0 (animated 120 ms); color accent
while dragging else fgFaint; drag → `store.resizeSplit(split, gutter, delta, total)`
(saves); double-click → equalize the two neighbours via the same path with
`delta = (b-a)/2*total`.

**Empty area** (`root == null`): if a plugin screen is active → "nada aberto aqui" /
`"nada aberto do ${plugin.name}"` + optional home-command button; else EmptyPane
(drop target: drop anywhere = `store.select(tab)`; hint label "Trocar").

**EmptyPane text**:
- Headline: 0 live tabs → `Nenhum painel aberto`; 1 → `1 sessão rodando na lateral — clique nela pra trazer de volta`; n → `$n sessões rodando na lateral — clique numa pra trazer de volta`.
- `Arraste uma sessão da lateral pra cá — pela borda de um painel, ela divide a tela; pelo meio, toma o lugar dele` (width 330, centered).
- Cheat sheet (only if width ≥ keysWidth+14+210, keysWidth = 170 Linux / 104 macOS):
  rows for newClaude, newShell, newTask, nextPane, nextSession, closePane, settings:
  `chords.map(label).join("  ")` → `action.label`; plus `"${slot(0).label}…9"` →
  `Ir pra enésima sessão`.

**Drop zones on a pane** (`_PaneSlot._sideAt`, pointer = drag-feedback corner + grab offset):
```
dx = p.x - w/2 ; dy = p.y - h/2
core = 0.15 ; maxCore = 160
if |dx| < min(w*0.15,160) and |dy| < min(h*0.15,160): center
elif |dx/w| >= |dy/h|: dx<0 ? left : right
else: dy<0 ? top : bottom
if side != center and (side.axis==row ? w : h)/2 < minFor(side.axis): center
```
Hint overlay = the future rectangle (left/right half, top/bottom half, or whole),
accent 16% fill, 1.6 px accent 80% border, radius `Mx.radius = 10`, animated 110 ms
easeOut, centered pill label `Trocar` (center) / `Dividir` (sides).

**`store.dropTab(tab, target, side)`**:
1. tab == target or target not on screen → `focusPane(tab)`; return.
2. Different "screen" (plugin screen vs sessions) → `select(tab)`; return.
3. Remove tab from tree (`_drop`, may record a "hole", see below).
4. center → if target is pinned, tab.pinned = true; `swap(target → tab)`. Else `insert(tab, target, side)`.
5. focusedPaneId = tab; save; notify.

**`store.select(tab)`** (sidebar click): embedded → ignore; hibernated → `wake` (async);
if the screen is an active *group* and tab isn't in it and nothing pinned → tree = leaf(tab);
else `_place(tab)`. Save + notify.

**`_place(tab)`** (also used when a tab is created): if tree null → leaf(tab). Else if
tab not on screen: target = focused pane if unpinned (and not a plugin window being
kept), else first unpinned open pane → `swap`. If none: plugin-window special case
inserts to the right of the focused one; else try to refill the last "hole"
(`_fillHole`: valid only if the current tree is the saved tree minus the hole); else
`edge(tab, right)`. Always `focusedPaneId = tab`.

**`_drop(tab)`**: remove from parked trees; if tab is unpinned and on screen, snapshot
tree; `remove`; if any pinned pane remains, remember hole `(snapshot, tab.id)`. If the
focused pane was removed, focus `left[clamp(oldIndex, 0, len-1)]` or null.

**`dismiss(tab)`** (header X): `_drop` only — session keeps running. **`closeTab(tab)`**:
also closes embedded terminals of a plugin window, disarms, emits `session.closed`
(non-passive), `kill()` unawaited, removes from `tabs`, `_drop`; if tree becomes empty,
places the last remaining tab of the same screen. **`closeFocused`** = closeTab(focusedTab).

**Focus**:
- `focusedTab` = tab(focusedPaneId) ?? first in `order`.
- `focusPane(tab)`: only if on screen and different → set + notify. Triggered by any
  pointer-down inside a pane (raw pointer listener, before gesture arena — so clicks
  in the terminal body also move the ring).
- `cyclePane(±1)`: needs ≥2 panes; wraps.
- `cycle(±1)`: walks *all* tabs of the current screen (sidebar order), wrapping, via `select`.
- `selectIndex(i)` (slot keys): `select(tabs[i])` if in range.
- Keyboard follows focus: when a TerminalPane becomes `focused` (false→true) it requests
  keyboard focus immediately; a newly mounted focused pane requests focus **after the
  first frame** (so it wins over dialogs returning focus to the previous owner and over
  the sidebar search field). Embedded terminals (inside plugin windows) take focus on
  pointer-down.
- Focus ring: drawn only when `showFocus && focused`: accent at 65% alpha as a 2 px
  foreground border; otherwise the tint color (3 px bottom border on header) or `Mx.border`.

### 2.4 Mouse wheel / trackpad

Wheel events are intercepted **above** the terminal view; the view's own scroll
simulation is disabled (`simulateScroll: false`) and the emulator's mouse handler is
wrapped to **never** report wheel buttons (returns "not handled" for wheel), while
still reporting clicks/drags normally.

- **Main screen:** the view's own scrollback scrolling handles the wheel (normal
  scrollback viewport scrolling). Nothing sent to the pty.
- **Alt screen:**
  ```
  pending += dy                    # wheel: scrollDelta.dy ; trackpad pan: -panDelta.dy
  notches = trunc(pending / lineHeight)   # toward zero
  if notches == 0: return
  pending -= notches*lineHeight
  cell = cellAt(pointer) clamped to [0,cols-1] x [0,rows-1]
  repeat |notches|: session.wheel(up: notches<0, col, row)
  ```
  `pending` resets to 0 on trackpad pan start.
- `wheel(up, col, row)`:
  - If the program did not enable any mouse reporting (`mouseMode.reportScroll` false)
    → send cursor key **Up/Down** (respecting DECCKM: `\x1b[A` or `\x1bOA`).
  - Else button = 64 (up) / 65 (down); x = col+1, y = row+1; encoding per mouse report mode:
    - SGR (1006): `\x1b[<{b};{x};{y}M`
    - urxvt (1015): `\x1b[{b+32};{x};{y}M`
    - normal / UTF-8: `\x1b[M` + chr(32+b) + chr(32+x) + chr(32+y)

### 2.5 Clicks & links

- Primary button pointer-down records the position; pointer-up within **4 px**
  (`_clickSlop`) is a click (else it was a drag-select).
- Click resolution: `href = oscLinks.at(cell) ?? linkAtCell(terminal, cell, base: tab.cwd)`.
  If found → `store.followLink(href, from: tab)`. Otherwise nothing (the click only focused the pane).
- Right-click (secondary tap) on the terminal → context menu with two items:
  `Copiar` (icon content_copy, chord label `MxChord.copy.label`, enabled iff a selection
  exists) and `Colar` (chord label `MxChord.paste.first.label`). Copy → `copySelection`,
  paste → `pasteClipboard(imagesViaCtrlV: kind==claude)`.
- Right-click on the pane header → panel menu (separate spec).

#### `linkAt(line, column, base?)` — heuristic links (exact)

Constants:
```
_breaks    = ' \t "\'`<>|()[]{},;*'
_tail      = '.,;:!?\'"'
_url       = /(?:https?:\/\/|mailto:)\S/
_atLine    = /^(.+?):\d+(?::\d+)?$/
_urlBreaks = _breaks without '(' and ')'
```
Algorithm:
1. column out of range or `line[column]` ∈ _breaks → null.
2. `_urlAt(line, column)`; if non-null return it.
3. Expand token around column bounded by _breaks. If token matches `_url` → null
   (a URL that `_urlAt` rejected is not a path).
4. `bare = trimTail(token)`; `path = trimTail(atLine.group1(bare) ?? bare)`.
5. If path has no `/` and is not markdown (`.md`, `.markdown`, `.mdx`, case-insensitive
   on basename) → null.
6. If `File(resolveLinkPath(path.split('#')[0], base)).exists` is false → null.
7. Return `path` (relative form, not resolved).

`_urlAt(line, column)`:
1. Expand word around column bounded by `_urlBreaks`.
2. First `_url` match inside the word; none → null. `start = wordStart + match.start`.
3. Scan `end` from start to wordEnd inclusive with depth=0: `(` → depth++;
   `)` → if depth==0 break else depth--; other char in `_breaks` → break.
4. If column ∉ [start, end) → null.
5. `trimmed = trimTail(line[start:end])`; return trimmed if it still matches `_url` else null.

`trimTail` strips trailing chars in `_tail` repeatedly.

`resolveLinkPath(path, base)`: expand `~`/`~/` with `$HOME` (avoiding `//`); absolute
→ as is; else `base/path` if base non-empty else path.

`linkAtCell(terminal, cell, base)`: null if `cell.y` out of buffer. Find the logical
line: walk up while line `isWrapped`, down while next `isWrapped`. Build text cell by
cell for full width (`cols`) of each physical line; codepoint 0 **or > 0xFFFF** →
space (keeps column ↔ string index aligned). Column = `(cell.y-first)*cols + cell.x`.
Delegate to `linkAt`.

#### OSC 8 links (`TermLinks`)

Hooked into the emulator's private-OSC callback (xterm forwards unknown OSC as
`(code, args)` split by `;`).

- `read(code, args)`: ignore unless code == "8". `url = args.length < 2 ? "" : args[1..].join(";")`
  (URI may contain `;`). Always **close** any open span first. If url empty → return.
  Else open: remember url, whether the alt buffer is active, and an anchor at the cursor.
- `close()`: if nothing open → return. If start anchor detached or buffer
  (alt/main) changed → discard. End anchor = cursor. `label = labelBetween(from, to)`;
  empty → discard. Else store span `{url, from, to, label, alt}`; keep at most **512**
  spans (drop oldest).
- `labelBetween(from, to)`: empty if `to <= from`; else cells row-major from `from`
  (inclusive) to `to` (exclusive), full width per middle row, null cell → space.
- `at(cell)`: iterate spans newest → oldest: detached span (line scrolled out) → delete;
  span of other buffer or not containing cell → skip; current label ≠ stored label
  (TUI redrew) → delete; else return `localPath(url)`.
- `holds(cell)`: from (inclusive) to (exclusive) in row-major order.
- `localPath(url)`: `file://` with empty host or `localhost` and non-empty path →
  filesystem path; otherwise url unchanged.

#### `store.followLink(href, from, base)`

1. Empty/blank → nothing.
2. Scheme http/https/mailto → open with system handler; failure → sticky banner
   `Não consegui abrir $href`.
3. `path = href.split('#')[0]`; empty → nothing. `target = resolveLinkPath(path, base ?? from.cwd)`.
4. Missing → sticky banner `Esse link aponta pra um arquivo que não existe: $target`.
5. Markdown → open in reader pane beside the source (`showFile`). Else Quick Look
   (`Notifier.quickLook`; on Linux, the system opener).

### 2.6 Selection, copy

- Selection is the emulator's: mouse drag selects (cell-precise, must land on the row
  under the pointer even after `ESC[3J`), double-click selects a word (xterm default
  word separators), select-all via shortcut (§3.3). Selection colour = accent at 30%
  alpha. Cursor colour = accent.
- `selectedText(terminal, range)` (exact):
  ```
  sel = normalized(range); lines = [""]
  for each segment (row, start?, end?) of sel:
    skip rows outside [0, bufferHeight)
    if not (row == sel.begin.y or row == 0 or line.isWrapped): lines.append("")
    lines.last += segmentText(line, start, end)
  return lines.join("\n")

  segmentText(line, start, end):
    from = max(start ?? 0, 0) ; to = min(end ?? line.length, line.length)
    drawn = to ; while drawn > from and cp(drawn-1) == 0: drawn--     # trailing never-drawn cells dropped
    for i in from..drawn-1:
      if cp(i) == 0:
        if i>0 and width(i-1) == 2: continue                         # tail of a wide char
        out += ' '; continue
      if i + width(i) <= to: out += char(cp(i))                      # wide char cut in half is excluded
  ```
  Spaces actually written by the program are kept anywhere; only never-written cells
  at the end of a row are trimmed.

### 2.7 Scrolling & scrollback

- Scrollback 8000 lines, main screen only; alt screen has none of its own (Claude's
  conversation scrolls only through wheel reports, §2.4).
- Viewport stays pinned to the bottom on new output (xterm default) unless the user
  scrolled up.

### 2.8 Search

There is **no in-terminal search** in this version. The theme defines search-hit
colours (`searchHitBackground: yellow`, `searchHitBackgroundCurrent: accent`,
`searchHitForeground: canvas`) but nothing calls the emulator's search/highlight.
`MxAction.search` focuses the **sidebar** search field (§3.2). Rust: implement nothing
here for parity; keep the palette slots.

### 2.9 Features of xterm relied upon (checklist for the Rust engine)

| Feature | Used? | Notes |
|---|---|---|
| Scrollback (8000) | yes | main screen only |
| Alt screen 1047/1049 | yes | with attribute restore fix |
| Mouse reporting modes 1000/1002/1003, encodings 1005/1006/1015 | yes | buttons by emulator; wheel by app (§2.4) |
| Bracketed paste `?2004` | yes | `paste()` wraps in `\e[200~ … \e[201~` when enabled |
| DECCKM application cursor keys | yes | wheel fallback & key encoding |
| Keyboard → bytes keytab (arrows, F-keys, Ctrl+letter → C0, Alt → ESC prefix) | yes | except the overrides in §3.3 |
| IME / composed text input | yes | with dedupe §3.5 |
| Selection (drag, double-click word, select-all) | yes | |
| Reflow on resize | yes | |
| OSC 8 hyperlinks | yes | via private-OSC hook (§2.5) |
| Device attribute replies (`CSI c`, `CSI > c`, DSR) | yes | must reach the pty via onOutput |
| 256-colour and truecolor SGR | yes | palette = theme ANSI 16 |
| Bold/faint/italic/underline/inverse | yes | |
| Window title (OSC 0/2), icon name | **no** | callbacks not wired |
| Bell (BEL) | **no** | not wired |
| Search / highlight | **no** | §2.8 |
| Cursor blink | default | xterm default |
| Font | Hack | §2.10 |

### 2.10 Font, theme, zoom

- Default face **Hack** v3.003 bundled (Regular, Bold 700, Italic, BoldItalic).
  `MxType.standard = ('Hack', 13.0 px, line height 1.2)`. Size clamped 9–24, line 1.0–1.8.
- Fallback chain (in order): `Menlo, Monaco, STIX Two Math, Apple Symbols,
  Noto Sans Symbols 2, Noto Sans Symbols, DejaVu Sans, Noto Color Emoji, Liberation Mono,
  Courier New, monospace, sans-serif` (Hack lacks `⏺ ✽ ⎿ ⏱ ☒`, Claude Code's markers).
- Per-pane zoom: `tab.zoom` integer steps of 1 px; effective size = `base + clampZoom(zoom)`,
  `clampZoom = clamp(zoom, ceil(9 - base), floor(24 - base))`. Persisted per pane as
  `"zoom"` when ≠ 0. `zoomFocused(±1)` and `resetZoomFocused()` act on the focused tab
  (readers too). Cache style objects per `(face, size, line)` to avoid relayout.
- Terminal theme: cursor = accent; selection = accent@30%; fg/bg = palette fg/bg (the
  terminal background is the pane background); ANSI 16 from the theme.

### 2.11 Pane header (`_PaneHeader`) — 42 px tall

Left→right: Claude avatar (claude) or 8 px status dot (others) · 9 px · block:
- Title (`paneTitleStyle` 15 px w700, ellipsis), then `PaneKeyHint` (`slot(index).label`,
  mono 10.5 px faint; hidden if index ∉ [0,8]; index = position in `store.tabs`).
- Only when the block is ≥ 170 px wide: chips (project name purple w/ track_changes
  icon; branch if non-empty and ≠ title; context chip for claude with context; dirty
  count yellow; ResultChip if files touched; FollowUpMark) then subtitle (12 px fgDim,
  flex 2).
- Buttons (26×26, icon 16): relaunch (`Rodar ${launcher.command} de novo`, when launcher
  and exited) · wake (`Retomar a conversa`, when hibernated) · agent handle (fingerprint)
  · pin (when pinned or paneCount > 1) · `mostrar na lateral` · done toggle
  (`Concluída — clique pra reabrir` / `Marcar esta sessão como concluída`) · close
  (`Tirar do painel — a sessão continua na lateral` + `\n${chord.label} encerra a sessão`
  using the first closePane chord).
- When `showFocus && !focused`: chips, subtitle and buttons at opacity 0.55; title is
  **never** dimmed.

Agent handle click: `held = Linux ? Ctrl : Alt`; `wantsId = held || name == null`;
copy id or name. Banners: `Id da sessão copiado: $value`; name with no live twin:
`Nome copiado: $value — outro agente fala com esta sessão por esse nome`; with twins:
`Nome copiado: $value — cuidado: mais $twins sessão(ões) atendem por esse mesmo nome`.
Tooltip lines: `Nome: $name  ·  clique pra copiar`, `Sessão: $id  ·  Ctrl+clique|⌥ clique pra copiar`.

Context chip: colour red ≥ 85, yellow ≥ 60, else fgDim; 5 segments 4×7 px, filled =
`clamp(ceil(percent/20), 0, 5)`; text `${percent}%`; tooltip
`${formatTokens(tokens)} de ${formatTokens(window)} tokens`.

`formatTokens(n)`: `n < 1000` → `"$n"`; `t = round(n/1000)`; `t < 1000` → `"$t mil"`;
else `d = round(n/100000)`; `d%10==0` → `"${d/10} mi"` else `"${d/10},${d%10} mi"`.

Under the terminal (column, bottom): DocBar (30 px, markdown pills + `Plano`/`Plano ·N`),
then ResultStrip when open (max 186 px; `Arquivo alterado`/`Arquivos alterados`,
`Limpar`, newest first, `Mostrar no Finder`, gone files struck through, banner
`Esse arquivo não está mais lá: $path`). ResultChip: `1 arquivo`/`$n arquivos`, tooltip
`Ver o que mudou`/`Esconder o que mudou`. Strip open state is per pane widget and resets
when the pane is handed another tab. (Full detail lives with the hooks/doc specs.)

---

## 3. Keyboard

### 3.1 Chord model (`MxChord`)

`MxChord{key, meta, control, alt, shift}`; exact modifier match (⌘T does not fire on ⌘⇧T).
`linux` flag = host is Linux (mutable for tests).

- **label** — macOS: `⌃⌥⇧⌘` order + glyph; Linux: `Ctrl+Shift+Alt+Super+Key` order
  (`[control, shift, alt, meta]`).
  - mac glyphs: tab `⇥`, enter `⏎`, space `␣`, backspace `⌫`, delete `⌦`, escape `⎋`,
    arrows `← → ↑ ↓`, home `↖`, end `↘`, pageUp `⇞`, pageDown `⇟`, F1–F12 `F1`…; others
    = keyLabel uppercased.
  - Linux glyphs: `Tab, Enter, Space, Backspace, Del, Esc, Home, End, PgUp, PgDn, Insert`;
    arrows stay arrows; others = mac glyph rules.
- **id** (disk) — `[ctrl, alt, shift, meta, name].join('+')`; names: `tab enter space
  backspace delete escape left right up down home end pageup pagedown f1…f12`,
  `plus` for the `+` key; printable keys = lowercase keyLabel (e.g. `t`, `=`, `,`, `0`).
- **parse(id)** — null for null/empty; last part = key (1 char → that codepoint as a
  logical key, including non-table keys like `ç`; else name lookup); any other part
  not in `{ctrl, alt, shift, meta}` → null.
- **forHost** — Linux and meta → `{key, ctrl, shift, alt}` (meta dropped). Used for
  plugin manifest chords.
- **slot(i)** — key `'1'+i`; macOS meta, Linux alt.
- **copy** — mac ⌘C; Linux Ctrl+Shift+C.
- **paste** — mac [⌘V]; Linux [Ctrl+Shift+V, Ctrl+V, Shift+Insert] (first is the one menus show).
- **rejection** (null = allowed), in order:
  1. modifier-only key → null (not an error; `valid` is false though).
  2. unknown key (not in name table and keyLabel length ≠ 1) → `essa tecla não dá pra usar num atalho`.
  3. reserved (below) → its reason.
  4. equals any slot(0..8) → `é a tecla da ${i+1}ª sessão da lateral`.
  5. no meta/ctrl/alt and not F1–F12 → Linux `precisa de Ctrl ou Alt — sem modificador a tecla sumiria do terminal`; mac `precisa de ⌘, ⌃ ou ⌥ — sem modificador a tecla sumiria do terminal`.
  6. ctrl only (no shift/meta/alt) + letter a–z → `o terminal manda $label pro processo como código de controle`.
- `valid = !isModifier && rejection == null`. Modifiers: meta/control/alt/shift (L/R/any), capsLock, fn.
- `fromEvent(event)`: only key-down; modifiers read from current hardware state.

Reserved — macOS:

| Chord | Reason |
|---|---|
| ⌘Q | `o macOS encerra o app com ⌘Q` |
| ⌘W | `o macOS fecha a janela com ⌘W` |
| ⌘H | `o macOS esconde o app com ⌘H` |
| ⌘M | `o macOS minimiza a janela com ⌘M` |
| ⌃⌘F | `o macOS usa ⌃⌘F pra tela cheia` |
| ⌥⌘H | `o macOS usa ⌥⌘H pra esconder os outros apps` |
| ⌘V | `o painel cola com ⌘V` |
| ⌘C | `o painel copia a seleção com ⌘C` |

Reserved — Linux:

| Chord | Reason |
|---|---|
| Ctrl+Shift+C | `o painel copia a seleção com Ctrl+Shift+C` |
| Ctrl+Shift+V | `o painel cola com Ctrl+Shift+V` |
| Ctrl+Shift+E | `o IBus abre o seletor de emoji com Ctrl+Shift+E` |
| Ctrl+Shift+U | `o IBus usa Ctrl+Shift+U pra digitar um caractere pelo código` |
| Ctrl+Alt+T | `o desktop abre um terminal com Ctrl+Alt+T` |
| Ctrl+Alt+Del | `o desktop usa Ctrl+Alt+Del` |
| Ctrl+Alt+←/→/↑/↓ | `o desktop troca de área de trabalho com Ctrl+Alt+setas` |
| Alt+F4 | `o desktop fecha a janela com Alt+F4` |
| Alt+Tab | `o desktop alterna as janelas com Alt+Tab` |

(Ctrl+V and Shift+Insert are not reserved on Linux, but paste intercepts them in the terminal.)

### 3.2 Actions and default bindings (`MxAction`, enum order = priority order)

| id | label | hint | group | macOS defaults | Linux defaults | action |
|---|---|---|---|---|---|---|
| `claude` | Nova sessão Claude | Na pasta e na feature/hotfix do painel em foco | Sessões | ⌘T | Ctrl+Shift+T | `openClaude(focusedFolder, cwd: folder.root, project: focusedProject)` |
| `shell` | Novo shell | Um terminal comum, no mesmo lugar | Sessões | ⇧⌘T | Ctrl+Shift+Enter | `openShell(focusedFolder, project)` |
| `task` | Nova task | Abre a worktree e sobe uma sessão nela | Sessões | ⌘N | Ctrl+Shift+N | loose folder → banner `Nova task precisa de uma pasta — o painel em foco não está em nenhuma`; else new-task dialog |
| `rename` | Renomear o painel em foco | O título dele; em branco, volta a ser o da branch ou da pasta | Sessões | ⌘E | F2 | no focused → banner `Nenhum painel em foco pra renomear`; else dialog `Renomear painel` |
| `pin` | Prender o painel em foco | A lateral para de trocá-lo; de novo, solta | Sessões | ⌥⌘P | Ctrl+Alt+P | no focused → `Nenhum painel em foco pra prender`; else `togglePin` |
| `close` | Fechar o painel em foco | — | Sessões | ⌘⌫ | Ctrl+Shift+W | `closeFocused()` (kills the session) |
| `close-settled` | Fechar as sessões resolvidas | As que já foram marcadas como prontas | Sessões | ⇧⌘K | Ctrl+Shift+K | `closeSettled(focusedFolder)` |
| `next-pane` | Próximo painel | Na ordem em que estão na tela | Navegação | ⌃⇥, ⌥⌘↓ | Ctrl+Tab, Ctrl+PgDn | `cyclePane(1)` |
| `prev-pane` | Painel anterior | — | Navegação | ⌃⇧⇥, ⌥⌘↑ | Ctrl+Shift+Tab, Ctrl+PgUp | `cyclePane(-1)` |
| `next-session` | Próxima sessão | Percorre a lateral inteira, painel aberto ou não | Navegação | ⌥⌘→ | Ctrl+Shift+→ | `cycle(1)` |
| `prev-session` | Sessão anterior | — | Navegação | ⌥⌘← | Ctrl+Shift+← | `cycle(-1)` |
| `search` | Buscar na lateral | Foca o campo de busca; Esc limpa e devolve o teclado | Navegação | ⌘F | Ctrl+Shift+F | if sidebar hidden or plugin view shown: leave plugin view, unhide sidebar, then focus search after next frame; else focus search |
| `markdown` | Abrir um markdown… | Escolhe um arquivo e o desenha num painel de leitura | Sessões | ⌘O | Ctrl+Shift+O | `openMarkdown()` |
| `plan` | Ver o plano da sessão em foco | Abre o último plano num painel de leitura, ao lado dela | Sessões | ⇧⌘L | Ctrl+Shift+L | focused not claude → banner `O painel em foco não é uma sessão do Claude`; else `showPlan` |
| `refresh` | Reler o git | Worktrees e branches, sem esperar os dez segundos | Janela | ⌘R | Ctrl+Shift+R | `refreshGit()` |
| `zoom-in` | Aumentar o corpo do painel | Só o painel em foco; a base de todos está em aparência | Exibição | ⌘=, ⇧⌘= | Ctrl+=, Ctrl+Shift+=, Ctrl+Shift+plus | `zoomFocused(1)` |
| `zoom-out` | Diminuir o corpo do painel | — | Exibição | ⌘- | Ctrl+- | `zoomFocused(-1)` |
| `zoom-reset` | Voltar o painel ao corpo base | O 0 fica livre: do 1 ao 9 são as nove sessões da lateral | Exibição | ⌘0 | Ctrl+0 | `resetZoomFocused()` |
| `sidebar` | Esconder ou mostrar a lateral | A aba na borda esquerda a traz de volta, e esta tecla também | Exibição | ⌥⌘S | Ctrl+Shift+B | `toggleSidebar()` |
| `settings` | Configurações | Esta tela | Janela | ⌘, , ⇧⌘P | Ctrl+, , Ctrl+Shift+P | open settings |

Enum order (also `MxKeymap.load` dedupe priority): claude, shell, task, rename, pin,
close, close-settled, next-pane, prev-pane, next-session, prev-session, search,
markdown, plan, refresh, zoom-in, zoom-out, zoom-reset, sidebar, settings.
Group display order: Sessões, Navegação, Exibição, Janela.
`dictate` (`⇧⌘D`) is commented out (§4); a saved `dictate` key is ignored.

Fixed (not configurable) keys:

| Keys | Where | Action |
|---|---|---|
| slot 1–9: ⌘1…⌘9 (mac), Alt+1…Alt+9 (Linux) | window map | `selectIndex(i)` |
| copy: ⌘C / Ctrl+Shift+C | terminal | `copySelection` |
| paste: ⌘V / Ctrl+Shift+V, Ctrl+V, Shift+Insert | terminal | `pasteClipboard(imagesViaCtrlV: claude)` |
| Shift+Enter | terminal | `newline()` → `\x1b\r` |
| ⌘A (mac, xterm default) / Ctrl+Shift+A (Linux) | terminal | select all |
| Esc (no ⌘/⌃/⌥) | shortcut recorder | cancel recording |

### 3.3 Dispatch order

**Inside a focused terminal** (`onKeyEvent`, runs before the emulator's keytab):
1. Shift+Enter → newline; handled.
2. Any paste chord → paste; handled.
3. Copy chord → copy; handled.
4. `keymap.match(event)` (first action in enum order with any matching chord) → run; handled.
5. Plugin command whose chord hits (and the event is not matched by the keymap) → run on this tab; handled.
6. Otherwise not handled → the emulator encodes the key and writes it to the pty
   (Tab, Ctrl+C, arrows, etc. all reach the process).

The emulator's own shortcut table is replaced: macOS keeps only xterm's defaults (⌘C/⌘V
never reach it; ⌘A select-all remains); Linux has only Ctrl+Shift+A = select all
(xterm's Ctrl+V/Ctrl+A/Ctrl+Shift+C are removed so ^A and ^V reach the pty).

**Window level** (`_Keys`, events bubbling up from the focused widget):
1. Inner focus node (always present, autofocus, holds focus when no pane exists): plugin
   command for the event (excluding keymap matches) → run; handled.
2. Outer: `MxKeys.dispatch(bindings)`:
   - If the primary focus is a text field (EditableText or descendant) and the key is in
     the editing set `{backspace, delete, ←, →, ↑, ↓, home, end, A, C, V, X, Z}` → ignored
     (lets ⌘⌫ delete-to-line-start in the sidebar search instead of closing the pane).
   - Else the **first** binding that matches wins (actions in enum order, then slots).

Gap to note: slot keys are not checked inside the terminal handler; on macOS the ⌘
digits bubble (xterm does not claim ⌘ combos). On Linux, Alt+digit inside the terminal
may be consumed by the emulator as `ESC digit`. **Rust: intercept slot chords in the
terminal handler too**, between step 4 and 5, to match the evident intent.

Plugin chord matching (`chordHits`): logical match, or — for letters a–z and digits —
physical key match by USB HID usage (`a..z` = 0x04..0x1D, `1..9` = 0x1E..0x26, `0` =
0x27) with exact modifier state. Plugin manifest chords pass through `forHost`.

### 3.4 Keymap model and config format

`MxKeymap` = map action → ordered list of chords (starts with defaults).

- `owner(chord)`: first action (enum order) containing it.
- `bind(action, chord, replacing?)`: remove chord from its previous owner; if `replacing`
  is in the action's list, replace it in place; else append if absent. Returns the
  previous owner if it was a different action (store shows banner
  `${chord.label} saiu de "${stolen.label}"`).
- `unbind`, `reset()`, `resetAction(action)` (first strips each default chord from any
  other owner, then restores defaults), `isDefault`, `allDefault`.
- **Persistence** in `$HOME/.maestria/config.json` under key `"shortcuts"`, only
  present when non-empty; contains only actions whose list differs from defaults:
  ```json
  "shortcuts": { "refresh": ["meta+r", "f5"], "shell": [] }
  ```
  Load: for each action whose value is a list: keep string items that parse and are
  `valid`, replacing that action's list (unknown items dropped silently). Then dedupe:
  walking actions in enum order, a chord already seen is removed from later actions.
  Non-map value → ignored.

Settings screen (Atalhos tab): heading `Atalhos`, hint `Clique numa tecla pra trocar, no + pra dar uma segunda combinação à mesma ação. Uma tecla só tem um dono: dar a outra ação tira de quem a tinha.`, button `Restaurar padrões` when not all default.
Recorder shows `Pressione a combinação   ⎋ cancela`, swallows every key while active;
modifier-only presses keep waiting; Esc without ⌘/⌃/⌥ cancels; a rejected chord shows
the reason (yellow, block icon) and stays recording. A chord cap can be removed only if
the action has more than one chord.

### 3.5 Text input / IME dedupe (`VtTerminal.textInput`)

State: `_unspentKey` (bool), `_lastInput` (string?).

- A **raw hardware key-down or repeat** (not key-up) observed while the terminal has
  focus — observed globally, *before* the focus system, and never consumed — sets
  `_unspentKey = true`.
- `textInput(text)`: if empty → drop (do not spend the key). If `!_unspentKey && text == _lastInput`
  → drop (ibus echo). Else `_unspentKey = false; _lastInput = text`; forward.
- `send()` and `paste()` set `_lastInput = null`.
- Composition in progress (preedit) never reaches the pty; only the commit does.

### 3.6 Other keyboard details

- Ctrl+Tab is a shortcut (next pane), so it never reaches the pty; plain Tab does.
- `Shift+Enter` must not be sent as `\r` (xterm's keytab ignored shift).
- On macOS the app removed the Edit menu's ⌘C/⌘V/⌘A key equivalents so AppKit does not
  swallow them before the view (Rust/winit: do not install menu accelerators for these).

---

## 4. Dictation (implemented but disabled in this version)

The entire feature is commented out in v2.4 ("fora desta versão"): no action, no UI, no
config key. A saved `"dictation"` config key is dropped on next save. Spec kept for
future re-enable.

**Flow** (`toggleDictation`, bound to `dictate` ⇧⌘D):
1. If phase == transcribing → ignore.
2. If busy (recording): `into = tab(target)`; `end()`; problem → sticky banner; empty
   text → banner `não entendi nada — nada foi colado`; target gone/exited → sticky
   banner `o painel que estava ouvindo não está mais aí: "$text"`; else
   `config.submit ? term.submit(text) : terminal.paste(text)`.
3. Else need a focused, non-reader, live tab, else banner
   `o ditado precisa de um painel com processo vivo em foco`; `begin(tab.id)`; failure → banner.

**Phases**: idle → recording → transcribing → idle. Header chip while active:
`Ouvindo…` (red, mic, pulsing 0.45↔1.0 over 750 ms, clickable → toggle, tooltip
`Parar e colar o que foi dito`) / `Transcrevendo…` (yellow, graphic_eq).

**begin(tabId)**: if busy → no-op. `problem()` check first:
- binary missing (`command -v <bin>` via login shell fails) →
  `` O ditado precisa do whisper.cpp: `brew install whisper-cpp` (ou `${bin}` no PATH) ``
- model file missing → `Falta o modelo em ${model} — ver configurações › ditado`.
Delete stale scratch wav `${systemTemp}/maestria-ditado-${pid}.wav`; set target & phase; record.

**Capture**:
- macOS: native method channel `maestria/mic`, methods `start(path)` / `stop()`
  (AVAudioEngine, so the TCC prompt is attributed to the app). Errors:
  `O microfone não abriu` / `O microfone não fechou` /
  `Este build não tem a metade nativa do microfone`.
- Linux: `Sh.shell -lc "exec <recorder> '<wav>'"` with recorder =
  `parecord --rate=16000 --channels=1 --format=s16le --file-format=wav` if `parecord`
  exists, else `arecord -q -f S16_LE -r 16000 -c 1 -t wav`; neither →
  `` O ditado precisa do `parecord` (pulseaudio-utils) ou do `arecord` (alsa-utils) ``.
  Stop = SIGINT (so the WAV header is finalized), wait up to 3 s, then SIGKILL.

**end()** (only from recording): phase=transcribing; stop recording; then:
- no file → `O microfone não gravou nada`
- size < `16000*2/2 + 44` = 16044 bytes (0.5 s) → `Não deu tempo de ouvir nada`
- peak |sample| (16-bit LE after 44-byte header) < 128 →
  `O microfone não captou som: gravou silêncio. Confira a entrada em Ajustes › Som › Entrada — um fone sem microfone no jack vira "Microfone Externo" e assume a entrada sem captar nada`
- transcribe; if `looping(text)` → `O que voltou foi ruído, não fala: "$text"`.
- finally: delete wav, reset to idle.
`cancel()`: stop, delete wav, reset.

**Transcription command** (run via login shell):
```
'<bin>' -m '<model>' -f '<wav>' -l '<lang>' [--prompt '<prompt>'] -nt -np
```
Non-zero exit → `O whisper não transcreveu` + `: <last non-blank stderr line>` if any.
Output → `clean(stdout)`: split lines, trim, drop empty and annotation lines matching
`^[\[(][^\])]*[\])][.]?$`, join with single space, trim.
`looping(text)`: lowercase, split on `[^\p{L}\p{N}#]+`, drop empties; true iff some word
repeats ≥ 4 times consecutively (and ≥ 4 words).

**Config** (`"dictation"` in config.json, only non-defaults):
`bin` default `whisper-cli`; `model` default `$HOME/.maestria/models/ggml-large-v3-turbo.bin`;
`lang` default `pt`; `prompt` default
`Ditado de instruções para o Claude Code. Vocabulário: worktree, branch, hotfix, feature, commit, rebase, merge, pull request, stash, deploy, endpoint, build, analyze, lint, TASK#48918, BUG#45902.`;
`submit` default false (written as `"submit": true` only when on). Blank bin/model/lang
→ default; blank prompt is kept as empty (disables `--prompt`).

---

## 5. Test cases (given / when / then)

Language-neutral restatement of the Dart tests touching this subsystem.

### 5.1 VT (vt_test)

| Given | When | Then |
|---|---|---|
| fresh terminal | write `ESC[1mA ESC[22mB` | A bold, B plain |
| fresh terminal | write `ESC[2mA ESC[22mB` | A faint, B plain |
| fresh terminal | write `ESC[?1049h ESC[4mTUI ESC[?1049l A` | A has no underline |
| scrubber+terminal | write scrub(`ESC[>4;2mABC`) | A, B, C have no attributes |
| scrubber | scrub(`ESC[>4mABC`) | `ABC` |
| scrubber | scrub(`a ESC[1mb ESC[22m ESC[38;5;208mc ESC[39m\r\n`) | unchanged |
| scrubber | scrub(`ESC[>c ESC[<u ESC[?25l A`) | unchanged |
| scrubber | scrub(`A ESC[>4`) then scrub(`;2mB`) | `A`, then `B` |
| scrubber | scrub(`A ESC`) then scrub(`[>4mB`) | `A`, then `B` |
| scrubber | scrub(`ESC[>` + 40×`9`) | returned unchanged (exceeds 32) |
| 80×10 terminal, 30 lines written | `ESC[3J` | anchor created at row 2 reports y=2; at row 9 reports 9 |
| same, anchor at row len-2 made before | `ESC[3J` | anchor.y == new len-2 |
| 80×10, alt screen, 20 lines | resize to 80×6 | anchor at row 1 reports 1 |

### 5.2 Paste (paste_test)

| Given | When | Then |
|---|---|---|
| clipboard text `git status` | paste (images via ^V on) | pty receives `git status` |
| clipboard has no text | paste, claude tab | pty receives `\x16` |
| clipboard has no text | paste, shell tab | nothing written |

### 5.3 Wheel (wheel_test)

| Given | When | Then |
|---|---|---|
| program set `?1049h ?1000h ?1002h ?1003h ?1006h` | wheel up then down at (0,0) | `ESC[<64;1;1M`, `ESC[<65;1;1M` |
| same | wheel down at col 41,row 7 | `ESC[<65;42;8M` |
| only `?1049h` | wheel up at (3,3) | single write `ESC[A` or `ESCOA` |
| mouse reporting on | emulator receives wheelUp mouse event | not handled, nothing written |
| mouse reporting on | emulator receives left-button down | handled, a report is written |

### 5.4 Typing / IME (typing_test) — writes observed at the pty

| Given | When | Then |
|---|---|---|
| focused pane | dead-key down, preedit `´`, key up | nothing written |
| …then | key E down (char é), commit `é`, up | `["é"]` |
| focused pane | `;` down (char ç), commit `ç`, up | `["ç"]` |
| dead key composed, A down+up, then commit `á` 3× | — | `["á"]` |
| compose `á` twice with full key sequences | — | `["á","á"]` |
| `ç` held: down+commit, then 3×(repeat+commit) | — | `["ç","ç","ç","ç"]` |
| type n, dead `~`, a→`ã`, o | — | joined `não` |

### 5.5 Selection (selection_test)

| Given | When | Then |
|---|---|---|
| 720×320 pane, 40 lines written, then `ESC[3J` | mouse drag from cell (0,3) to last char of row 3 | selection begin.y = end.y = 3; copied text == row 3 text |
| 40 lines, no clear | drag over row len-3 | begin.y == that row; text equal |

### 5.6 Links (links_test)

| Given | When | Then |
|---|---|---|
| `veja https://docs.anthropic.com/claude-code depois` | linkAt cols 5, 20, 42 | the URL |
| `abre https://x.dev/a.` | col 10 | `https://x.dev/a` |
| `│ https://x.dev/a` | col 12 | `https://x.dev/a` |
| `(veja https://x.dev/a)` | col 10 | `https://x.dev/a` |
| `veja https://en.wikipedia.org/wiki/Rust_(programming_language).` | col 10 and col of `programming` | `…Rust_(programming_language)` |
| `(veja https://en.wikipedia.org/wiki/Rust_(x))` | col 10 | `…Rust_(x)` |
| `escreva pra mailto:eu@marrow.com.br` | col 20 | `mailto:eu@marrow.com.br` |
| `pegue em ftp://x.dev/a` | col 15 | null |
| `escrevi o middleware e/ou o resto` col 22; `nada aqui` col 3; `` col 0; `abc` col 9 | — | null |
| dir with lib/store.dart | `escrevi lib/store.dart agora` col 12, base=dir | `lib/store.dart` |
| same | `lib/sumiu.dart` | null |
| same, no base | — | null |
| `veja lib/store.dart:42:9` col 10 | base=dir | `lib/store.dart` |
| `veja <dir>/PLANO.md:3` col 10 | — | `<dir>/PLANO.md` |
| `atualizei o PLANO.md` | col 14 / col 3 | `PLANO.md` / null |
| buffer `rode isto:\r\nabra https://x.dev/auth em outra aba` | linkAtCell (10,1) / (3,0) | URL / null |
| 40-col terminal, long URL wrapped | linkAtCell (5,1) and (10,0) | full URL |
| `https://x.dev/a` | linkAtCell y=-1 / y=len+5 | null |
| tab cwd=dir | followLink(`PLANO.md`) | reader opened for `<dir>/PLANO.md`, paneCount 2 |
| tab cwd=dir | followLink(`SUMIU.md`) | no reader; banner contains `não existe` |
| `abra ` + OSC8(`https://x.dev/auth`,`aqui`) + ` agora` | at (5,0),(8,0) / (4,0),(9,0) | URL / null; buffer text has no `x.dev`, contains `abra aqui agora` |
| 20-col, `leia `+OSC8(url,`um rótulo bem comprido`)+`!` | at (6,0), (3,1) / (7,1) | URL / null |
| OSC8(`file://<dir>/PLANO.md`) | at (2,0) | `<dir>/PLANO.md` |
| OSC8(`file://outra-maquina/etc/hosts`) | at (2,0) | unchanged URL |
| OSC8 label `aqui` then `\r ESC[2K outra coisa` | at (1,0) | null |
| 20×4, maxLines 30, link then 40 lines | at (1,y) for all y | null |
| VtTerminal 40×6, 20 lines, link on last line, `ESC[3J` | at (5,5) | URL; buffer has 6 lines |
| OSC8 a `um` then OSC8 b `dois` then close | at (0,0) / (3,0) | a / b |
| open c and immediately close, then `texto` | at (0,1) | null |
| OSC8 `https://x.dev/a?b=1;c=2` | at (1,0) | full URI with `;` |
| pane with `escrevi PLANO.md agora`, cwd has it | click cell (8,0) | reader opens `<dir>/PLANO.md` |
| pane with OSC8 file link labelled `o plano` | click (5,0) | reader opens it |
| same as first | click (2,0) | nothing opened, no banner |

### 5.7 Shortcuts (shortcuts_test)

| Given | When | Then |
|---|---|---|
| focused pane, text field focused with `procurando` | ⌘⌫ | tab still exists; field empty |
| no text field focused | ⌘⌫ | focused tab closed |
| ⌘T | label/id/parse | `⌘T`, `meta+t`, round-trips; every default of every action round-trips |
| ⌥⌘→ | label/id | `⌥⌘→`, `alt+meta+right`; ⌃Tab label `⌃⇥` |
| G alone / F5 alone | rejection | non-null / null |
| ⌃C / ⌃Tab | rejection | non-null / null |
| every reserved chord | rejection | non-null; ⌘Q's contains `⌘Q` |
| `hyper+t`, `meta+teclamágica`, ``, null | parse | null |
| defaults (mac) | — | no chord has two owners; none rejected; all actions non-empty; ids unique |
| Linux | labels | `Ctrl+Shift+T`, id `ctrl+shift+t`, slot0 `Alt+1`, `Ctrl+PgDn`, paste first `Ctrl+Shift+V` |
| Linux defaults | — | no meta; no duplicates; round-trip; none rejected; none equals copy/paste/slots |
| Linux | Ctrl+Shift+J / Ctrl+J / Ctrl+Shift+E / Ctrl+Alt+← | null / contains `Ctrl+J` / contains `IBus` / non-null |
| Linux | parse(`meta+alt+g`).forHost | Ctrl+Shift+Alt+G; `ctrl+alt+g` unchanged |
| Linux, focused panel | F2 | dialog `Renomear painel` |
| keymap | bind(shell, ⌘T) | returns claude; claude lacks ⌘T; owner(⌘T)=shell |
| keymap | bind(claude, ⌘J, replacing ⌘T) then bind(claude, ⌘T) | `[⌘J]` then `[⌘J, ⌘T]` |
| shell stole ⌘T | resetAction(claude) | claude = defaults; shell lacks ⌘T |
| fresh | toJson | `{}`; after bind(refresh, F5) → `{"refresh":["meta+r","f5"]}` |
| saved json | load into new keymap | same lists; allDefault false |
| load `{"claude":["meta+g"],"shell":["meta+g"]}` | — | claude `[⌘G]`, shell `[]` |
| load `{"claude":["meta+q","g","meta+alt+g"]}` | — | claude `[⌥⌘G]` |
| modified | reset() | allDefault, toJson `{}` |
| focused panel | ⌘E | dialog `Renomear painel` prefilled with title; entering `permissão do google` + `Ok` renames |
| no focused panel | ⌘E | no dialog; banner contains `renomear` |
| settings | open | `Aparência` visible with Nord theme label |
| settings › Atalhos | click `⌘T`, press ⇧⌘G | newClaude = [⇧⌘G]; cap shows `⇧⌘G` |
| recording | press G | unchanged; message contains `sem modificador`; still recording |
| recording | Esc | unchanged; recorder gone; dialog still open |

### 5.8 Pane focus (pane_focus_test)

| Given | When | Then |
|---|---|---|
| one pane | Enter | its pty gets `\r` |
| drop second tab to the right | Enter | second gets `\r`; first unchanged |
| sidebar search field focused | `select(tab)` | search loses focus; Enter → tab gets `\r` |
| dialog closed (returns focus) | `select(tab)` | Enter → tab gets `\r` |

### 5.9 Drop (pane_drop_test), window 2200×900, sidebar 660

| Drop point (fraction of pane) | Then |
|---|---|
| (0.95, 0.5) | order `[um, dois]`, 2 terminal panes |
| (0.68, 0.5) | order `[um, dois]`, row split |
| (0.5, 0.95) | order `[um, dois]`, column split |
| (0.5, 0.5) | order `[dois]` (swap) |
| after first split, `tres` near right edge of right pane | `[um, dois, tres]`, one row of 3 |
| window 1060 wide, (0.97, 0.5) | `[dois]` (too narrow → center) |

### 5.10 Pane tree (panes_test)

| Given | When | Then |
|---|---|---|
| leaf a | insert b right of a | row, `[a,b]`, `[0.5,0.5]` |
| leaf a | insert b top | column, `[b,a]` |
| row[a,b] | insert c right of b | `[a,b,c]`, `[0.5,0.25,0.25]`, all leaves |
| row[a,b] | insert c bottom of b | child 1 = column `[b,c]` |
| row[a,b,c] w `[.5,.25,.25]` | remove a | `[b,c]`, `[0.5,0.5]` |
| row[a,b] | remove b | leaf a |
| row[a, column[b, row[c,d]]] | remove b | row `[a,c,d]`, `[0.5,0.25,0.25]` |
| leaf a | remove a | null |
| row[a,b] | resize(0, +100, 1000) | `[0.6,0.4]` |
| row[a,b] | resize(0, −10000, 1000) | weights[0] ≈ 0.24 |
| row[a, column[b,c]@.7/.3]@.4/.6 | toJson/fromJson | same order & weights |
| row[a,b], b unsaved | toJson/fromJson | leaf a |
| store leaf a | dropTab(b, a, right) | `[a,b]`, focused b |
| row[a,b,c] | dropTab(a, c, right) | `[b,c,a]` |
| row[a,b] | dropTab(c, a, center) | `[c,b]`; a still in tabs, not exited |
| row[a,b] focused b | select(c) | `[a,c]`; b not open |

### 5.11 Pane header (pane_header_test) — selected

| Given | When | Then |
|---|---|---|
| 720 px pane | — | close button right edge < 12 px from pane right; same rect for short/long titles |
| one pane | tap close (tooltip `Tirar do painel — a sessão continua na lateral\n⌘⌫ encerra a sessão`) | tree null; tab alive |
| row[left,right] focused right | dismiss(left) | leaf right; focus right; both tabs exist |
| agentName + sessionId | tap fingerprint / Alt+tap | clipboard name / id |
| two live tabs share agentName | tap fingerprint | banner contains `cuidado` |
| two panes, this unfocused | — | title not under Opacity; close button under Opacity |
| one pane, index 0 | — | key hint `⌘1` |
| formatTokens | 999,1000,1499,1500,116010,116900,999499,999500,1000000,1040000,1050000,1200000,1950000,1999999,200000 | `999, 1 mil, 1 mil, 2 mil, 116 mil, 117 mil, 999 mil, 1 mi, 1 mi, 1 mi, 1,1 mi, 1,2 mi, 2 mi, 2 mi, 200 mil` |
| percent 0/1/20/21/100 | — | filled segments 0/1/1/2/5 |
| 58%, 116010/200000 | — | text `58%`, tooltip `116 mil de 200 mil tokens` |
| 59/60/84/85 | — | fgDim/yellow/yellow/red |
| context null, or shell tab, or pane 175 px wide | — | no chip |
| (tint tests belong to the theme/tint spec) | | |

### 5.12 Status line & context (statusline_test, context_usage_test)

| Given | When | Then |
|---|---|---|
| statusLine in home, then project settings.json, then settings.local.json | userStatusLine | `home` → `projeto` → `local` |
| invalid local JSON + home | — | `home` |
| statusLine type ≠ command | — | null |
| nothing / cwd null | — | null |
| no user statusLine | settingsFor(`t1`) | command args = `sh, <statusline.sh>, http://127.0.0.1:PORT/status/t1, ''`; no padding; hooks include SessionStart, Stop, PreToolUse, SubagentStop; Stop url `/hook/t1` |
| user statusLine with padding 2 | settingsFor | padding 2; arg 3 = base64(user cmd) |
| server started | — | script file content equals the constant |
| user cmd with quotes/$ | settings passed through `sh -c printf '%s' q(settings)` | JSON survives; decoded arg 3 equals user cmd |
| script, user `cat` | run with closed port | stdout = input JSON, exit 0, < 1.5 s |
| script, empty user | — | empty stdout, exit 0 |
| script, user with quotes/pipes | — | same output as running directly |
| script, live server | — | server receives input JSON |
| PATH without curl | — | stdout JSON, empty stderr, exit 0 |
| status payload used 58.4, usage 10+1000+115000 (+300 output) | fromStatusLine | 58 / 116010 / 200000 |
| used null; usage null; no context_window; non-map; size null/0; non-numeric percent; usage non-map; context_window non-map | — | null |
| server | POST /hook/tab9 Stop, POST /status/tab9 | one status event (tab9) and one hook event (Stop) |
| store | applyStatus valid / cleared / unknown tab | set / null / ignored |
| store | same valid payload twice | one notification |

### 5.13 Quick input (quick_input_test) — plugin dialogs, adjacent

| Given | When | Then |
|---|---|---|
| quick input `nome do grupo` | type `  AWS  ` + done | `AWS` |
| — | blank + done, or Esc | null |
| value `Staging` | open | text preselected 0..7 |
| no UI hook | plugin `window.input` | RPC error; with hook → title defaults to plugin name |
| form with required/half/select/showIf | submit empty → `falta host`; … | see plugin spec |

### 5.14 Dictation (dictation_test — suite commented out with the feature)

| Given | When | Then |
|---|---|---|
| ` abre a worktree\ndo BUG#45902\n e roda os testes ` | clean | `abre a worktree do BUG#45902 e roda os testes` |
| `[BLANK_AUDIO]` / `(música ao fundo)\nroda o analyze` | clean | `` / `roda o analyze` |
| `usa o hotfix (o da branch velha) pra comparar` | clean | unchanged |
| `\n  \n` | clean | `` |
| `Vocabulário: pinjate, pinjate, pinjate, pinjate, pinjate.` | looping | true |
| `não, não, não é isso`; `roda o analyze`; `a branch nova sai da branch main, não da branch velha` | looping | false |
| all-zero wav / speech wav / header-only | peak | 0 / > 20000 / 0 |
| model `/m/turbo.bin`, wav `/tmp/fala.wav`, lang pt | command | contains `-m '/m/turbo.bin'`, `-f '/tmp/fala.wav'`, `-l 'pt'`, `-nt`, `-np`, `--prompt` with `worktree`; blank prompt → no `--prompt`; spaces in path stay single-quoted |
| no binary / binary w/o model / both | problem | contains `brew install whisper-cpp` / contains model path not `brew` / null |
| config | toJson | defaults → `{}`; lang en → `{"lang":"en"}`; round-trips; blank bin/lang → defaults; blank prompt kept `''` and saved |
| toggle twice | — | recording then idle; pty gets `roda o analyze` with no `\r`; with submit → contains `\r` |
| focus moved mid-dictation | second toggle | text goes to the starting tab only |
| short wav / silent wav / looping text / empty text | — | nothing written; banners `Não deu tempo`, `não captou som`+`Entrada`, `ruído, não fala`, `não entendi nada` |
| no focused tab | toggle | mic not opened; banner mentions `painel` |
| after transcription / cancel | — | scratch wav deleted; cancel writes nothing, busy false |
| UI | — | `Ouvindo…` visible only while recording; settings show `Tudo no lugar` or `brew install whisper-cpp` |
| defaults | — | ⇧⌘D owned by dictate, no conflicts |

---

## 6. Recommended Rust approach

### 6.1 VT engine: `alacritty_terminal`

Use `alacritty_terminal` (0.24/0.25 line; pin exact) as the emulator core, fed by
`vte` through `alacritty_terminal::vte::ansi::Processor`.

How it covers the §2.1 fixes:

| Fix | alacritty_terminal |
|---|---|
| SGR 22 | correct natively (`CancelBoldDim`). Keep the test. |
| `CSI > … m` | vte reports the `>` as an intermediate/private marker; alacritty only applies SGR for `('m', no intermediates)` (newer versions route `>4;Nm` to modifyOtherKeys). **Keep a scrubber anyway** (cheap, byte-level, port §2.1 exactly) so behavior does not depend on crate version; it also matches the test vectors. |
| `?1049l` attribute restore | alacritty saves/restores the cursor (incl. template attrs) around 1049. Verify with the test; if it restores the saved *position* too, that matches real xterm and is acceptable — the test only checks attributes. |
| IME triple commit | UI-layer concern (§3.5); port the guard verbatim in the input module. |
| Blank cells on copy | `Term::selection_to_string` emits spaces for empty cells and trims trailing; verify wide-char spacer handling against §2.6 and the tests; otherwise implement `selected_text` over `Grid` cells directly (port §2.6). |
| `ESC[3J` anchors | alacritty's selection and points use grid lines relative to the viewport/history and stay consistent after `clear_history`. No reseat needed. |
| OSC 8 | stored per cell (`cell.hyperlink()`), so `TermLinks` reduces to "hyperlink of the clicked cell"; a TUI overwrite clears it automatically; scrolled-out lines take it away. Still apply `localPath` (`file://` → path). The 512-span cap disappears. |
| Mouse modes, bracketed paste, DECCKM | exposed as `TermMode` flags (`MOUSE_REPORT_CLICK`, `MOUSE_DRAG`, `MOUSE_MOTION`, `SGR_MOUSE`, `UTF8_MOUSE`, `BRACKETED_PASTE`, `APP_CURSOR`, `ALT_SCREEN`). The engine does **not** encode input — you write the key/mouse encoders (port §2.4 exactly; take keyboard encoding from alacritty's `input` module or `termwiz` as reference). |
| DA/DSR replies | delivered through the `EventListener` as `Event::PtyWrite(String)` — forward to the pty writer. |
| Title/bell | `Event::Title`, `Event::Bell` — ignore for parity. |

Scrollback: `Config { scrolling_history: 8000, .. }`. Reflow is built in.

### 6.2 PTY: `portable-pty`

- `native_pty_system().openpty(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })`.
- `CommandBuilder::new(shell)`; args `["-l"]` or `["-lc", format!("exec {cmd}")]`;
  `cwd(cwd)` **after** checking the directory exists (§1.7); `env_clear()` then set
  `Sh.env` + `FORCE_HYPERLINK=1` + caller env + explicit `TERM` (§1.2).
- `slave.spawn_command(cmd)`; drop the slave handle afterwards. `master.try_clone_reader()`
  on a dedicated thread → channel of byte chunks → `Utf8 lossy streaming decode`
  is unnecessary because vte consumes bytes (invalid UTF-8 becomes U+FFFD inside vte);
  apply the scrubber at byte level (all bytes it inspects are ASCII).
- `master.take_writer()` for input; `master.resize(PtySize)` on layout change.
- Signals: portable-pty's unix spawn calls `setsid` in the child, so the pid is a group
  leader. Use `nix::sys::signal::killpg(Pid::from_raw(pid), SIGHUP)` falling back to
  `kill(pid)`, then wait up to 3 s (child `try_wait` polling or a waiter thread),
  then `SIGKILL` (§1.8). `nix` keeps the crate `#![forbid(unsafe_code)]`-compatible.
- Exit: waiter thread `child.wait()` → `ExitStatus::exit_code()`; tag each spawn with a
  generation counter and ignore exits from stale generations (the `identical(_pty, pty)` rule).

### 6.3 Rendering the grid on a GPU UI

Whatever the app-wide UI choice (wgpu+winit with a custom compositor, or a framework
such as iced/gpui that exposes custom primitives), render each pane as:

1. **Snapshot** under the `FairMutex`/lock: iterate `term.renderable_content()`
   (display_iter of visible cells with resolved fg/bg/flags, cursor, selection range)
   into a compact Vec of `RenderCell { col, row, ch, fg, bg, flags, hyperlink? }`;
   release the lock before drawing.
2. **Background pass**: merge runs of equal bg per row into quads (instanced rect pipeline).
3. **Glyph pass**: shape per row with `cosmic-text`/`swash` (Hack + the §2.10 fallback
   chain via fontdb), glyph atlas in a texture (`glyphon` does this on wgpu);
   bold/italic faces, faint = fg at reduced alpha, underline/strike as rect quads,
   wide chars span 2 cells.
4. **Overlay pass**: selection (accent @ 30%), cursor (accent block/bar), hover underline
   for links if desired (not in Dart version).
5. Cell metrics from the font: `cell_w = advance('M')`, `cell_h = ceil(size * line)`;
   padding 12/10/12/10; cols = floor((w-24)/cell_w), rows = floor((h-20)/cell_h).
   Point→cell mapping must use **pane-local** coordinates (the Dart wheel bug).
6. Redraw on `Event::Wakeup` (coalesce to frame rate), on resize, theme, zoom.
7. Cache per-(face,size,line) metrics to avoid reshaping on every frame (§2.10).

### 6.4 Module layout (each file ≤ 400 LOC)

```
crates/maestria-term/src/
  lib.rs                 re-exports
  shell.rs               resolve_shell, quote(), env_with_fallbacks(), bin_dirs()    (~120)
  pty/
    mod.rs               PtySession: start_shell/start_command/relaunch/park/state    (~250)
    spawn.rs             CommandBuilder assembly, cwd check, env merge              (~120)
    signals.rs           hang_up(), kill_with_grace() (SIGHUP group→pid, 3s, SIGKILL)(~100)
    io.rs                reader thread, writer, generation-tagged exit watcher       (~150)
  vt/
    scrubber.rs          VtScrubber (byte-level port of §2.1)                        (~120)
    engine.rs            alacritty Term wrapper, EventListener → PtyWrite, resize    (~250)
    text.rs              selected_text(), logical_line_text() for link detection     (~200)
  input/
    keys.rs              key → bytes encoder (keytab), Shift+Enter → ESC CR          (~350)
    mouse.rs             button/wheel reports (SGR/urxvt/normal), arrow fallback     (~150)
    ime.rs               TextInputGuard (unspent key / last input), paste/send       (~100)
    clipboard.rs         copy_selection(), paste_clipboard(images_via_ctrl_v)        (~100)
  links/
    heuristic.rs         link_at(), url_at(), trim_tail(), resolve_link_path()       (~200)
    osc8.rs              hyperlink lookup + file:// → path                            (~80)
    follow.rs            follow_link() decision (url/system, md/reader, other/open)  (~100)
  layout/
    tree.rs              PaneNode, insert/remove/absorb/edge/swap/order              (~300)
    resize.rs            resize(), equalize                                           (~80)
    persist.rs           to_json/from_json (+ legacy active/split)                   (~150)
    drop.rs              side_at() drop-zone geometry                                 (~80)
  shortcuts/
    chord.rs             Chord, label (mac/linux), id, parse, for_host, slot         (~300)
    rules.rs             rejection(), reserved tables (mac/linux), modifier set      (~200)
    actions.rs           Action enum: id/label/hint/group/defaults mac+linux         (~300)
    keymap.rs            Keymap: bind/unbind/reset/reset_action/to_json/load         (~200)
    dispatch.rs          terminal-level and window-level dispatch order (§3.3)       (~200)
  status/
    statusline.rs        script constant, user_status_line lookup, settings fragment (~150)
    context.rs           ContextUsage::from_status_line, format_tokens               (~100)
  dictation/ (feature = "dictation", off by default)
    mod.rs               phases, toggle flow                                          (~250)
    record.rs            parecord/arecord + macOS native hook trait                  (~150)
    whisper.rs           command(), clean(), looping(), peak()                        (~200)
    config.rs            DictationConfig + json                                       (~120)
crates/maestria-ui/src/terminal/
  pane_view.rs           pane widget: header, terminal surface, doc bar, strip       (~350)
  surface.rs             pointer/wheel/click/focus handling                           (~300)
  render.rs              grid snapshot → GPU primitives                               (~350)
  header.rs              header chips/buttons/strings                                 (~350)
  panes_view.rs          tree rendering, grips, drop hint, empty pane                 (~350)
```

Keep the model (`layout`, `shortcuts`, `links`, `vt/scrubber`, `status/context`,
`dictation/whisper`) free of UI types so the §5 tables become plain `#[test]`s;
use `proptest` for `Panes` invariants (weights sum to 1 ± ε, no single-child splits,
no same-axis nesting after remove, `order` preserved by persistence round-trip) and
for the scrubber (output never contains `ESC[` + `<=>` … `m`; feeding a stream in
arbitrary chunk splits yields the same output as feeding it whole, for fragments ≤ 32).
