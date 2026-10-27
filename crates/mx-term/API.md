# mx-term — API

PTY + `alacritty_terminal`, no gpui. One I/O thread per live process; it reads the PTY,
runs the byte scrubber and the VT parser, and only flips a dirty flag (one `Wakeup` per
dirty period). Writes go straight to the non-blocking PTY fd from the caller's thread;
leftovers (full PTY buffer) are flushed by the I/O thread.

## Session — `TerminalSession` (cheap `Clone`, `Send + Sync`; implements `mx_core::ports::Term`)
- `new(GridSize, TerminalPalette, scrollback)`, `with_palette(p)` (80×24, `SCROLLBACK` = 8000),
  `spawn(SpawnOptions, size, palette)` (standalone program or `$SHELL -l`).
- Lifecycle (spec 02 §1): `start_shell_at(cwd)` (`<shell> -l`), `start_command_at(cmd, cwd, display)`
  (dim `$ display` note, then `<shell> -lc "exec cmd"`), `park_now`, `remark_now`, `relaunch_now`,
  `submit_text` (paste, 250 ms, `\r` if same life still alive), `kill_and_wait() -> Receiver<()>`
  (SIGHUP to group→pid, 3 s, SIGKILL), `hang_up_now`, `child_pid`, `is_exited`, `exit_status`.
  Missing cwd → red line, exit −1, hook fired. Any start on an exited term restarts it in place
  (scrollback kept). Exits are generation-tagged: a previous life's exit is ignored.
  Exit code = status code, or `-signo` when killed by a signal. Exit writes `[processo saiu com N]`.
- Env (spec 02 §1.2): `mx_sys::shell::env()` (PATH fallbacks) + `TERM=xterm-256color`,
  `COLORTERM=truecolor`, `LANG` only if absent, `FORCE_HYPERLINK=1`, then caller env.
- `set_exit_hook(Arc<dyn Fn(Option<i32>)>)`, `events() -> async_channel::Receiver<TermEvent>`
  (`Wakeup`, `Title`, `ResetTitle`, `Bell`, `ClipboardStore`, `Exited(code)`; shared, buffered from birth;
  pairs with `take_dirty()`), `same(&other)`.
- `subscribe() -> Subscriber{events, dirty: DirtyFlag}` — one per view: its own channel (dropping it
  unsubscribes) and its own dirty flag; one `Wakeup` per dirty period, re-armed by `dirty.take()`.
  Starts dirty. Views use this, never `events()`.
- A life's I/O thread and master fd live until the next start or the session drop, even after the leader
  exits (background jobs' output still shows); then the fd closes and the slave hangs up. Signals are sent
  under the state lock while the leader is still unreaped (no PID reuse).
- Input: `write(&[u8])` (raw; snaps viewport to bottom), `send(text)` / `newline()` (`ESC CR`) /
  `paste(text)` (bracketed when `?2004`; ESC stripped always, ^C and U+009B too inside brackets;
  clear the IME echo memory), `paste_clipboard(text, images_via_ctrl_v)
  -> PasteOutcome::{Text, CtrlV, Nothing}`.
- IME guard (spec 02 §3.5): `key_down(key)`, `key_up(key)`, `preedit(text)`,
  `input_text(text, TextOrigin::{Key, Commit, CommitKey}) -> bool` (writes only if the guard accepts).
  `CommitKey` = an IME commit delivered as its own key event (gpui Wayland `ime_commit`): never
  deduped against the previous text, only paired one-to-one with a real key's echo, either order.
- View: `resize(GridSize)` (term + `TIOCSWINSZ`, no-op when unchanged), `size`, `scroll(lines)`, `mode()`,
  `take_dirty()`, `snapshot_into(&mut Snapshot)` (reuses the buffer), `snapshot()`, `row_text(row)`,
  `set_palette(p)`, `feed(bytes)` (into the emulator through the scrubber; tests/notes).
- Mouse: `wheel(up, col, row)` (SGR/normal 64/65, 1-based, or arrow keys w/o mouse mode),
  `mouse_report(MouseButton, MouseAction, col, row, Mods) -> bool`.
- Selection: `start_selection(col, row, right_half, SelectionKind::{Cells, Word, Line})`, `update_selection`,
  `select_all`, `clear_selection`, `has_selection`, `selection_text(reflow)` (Dart `selectedText`: skipped cells →
  spaces, soft wraps joined; `reflow` also joins rows Claude wrapped itself and drops its indent/`●` — copy in
  claude tabs), `selected_text()` = `selection_text(false)`. Sticky: on the alt screen a selection follows its rows'
  text when the program repaints (30 ms / 120 ms grace); on the main screen one alacritty dropped is put back.
- Find: `find(query, limit) -> Vec<Hit{from, to: CellPos{x, y}}>` (smart-case literal, by logical line, `to` exclusive,
  `y` from the top of the scrollback), `viewport() -> Viewport{history, display_offset, rows}`, `scroll_to_top(y)`.
  `FindSession` (per open bar): `search(term, query, near_line)`, `refresh(term)` (current hit stays put),
  `step(term, ±1)` (wraps), `clear`, `hits`, `index`, `current`, `counter()` (`2 de 3`, `Nenhum`, `+` at `FIND_LIMIT`).
- Links: `link_at(col, row, base_cwd) -> Option<String>` — OSC 8 of the cell (`file://` → path) else the
  heuristic link of the logical (wrapped) line.

## Port — `TermRegistry` (implements `mx_core::ports::TermFactory`)
- `new(palette)`, `on_exit(Arc<dyn Fn(&str tab_id, Option<i32>)>)` — the app posts
  `StoreMsg::TermExited{tab_id, code}` from here (stale lives never reach it).
- `create(tab_id)` / `create_session(tab_id)`, `get(tab_id) -> Option<TerminalSession>` (weak; dropped
  sessions disappear and are hung up), `set_palette(p)` (all live sessions).

## Snapshot
`Snapshot{cols, rows, cells, cursor, foreground, background, mode, display_offset}`, `row(r) -> &[SnapCell]`;
`SnapCell{c, marks: Marks, style: CellStyle{fg, bg, bold, italic, underline, strikeout}, wide, spacer, selected}`
(`Copy`; `Marks::chars()` = up to 3 zero-width/combining chars drawn after `c`);
`SnapCursor{row, col, kind: CursorKind, color}`. Dim/inverse/hidden are already applied to colors.

## Pure helpers
- `encode_key(key, key_char, Mods, app_cursor)` (Shift+Enter → `ESC CR`, Alt+1…9 → `None`,
  Alt+key → `ESC` + the typed char, Ctrl+/ → 0x1f, xterm forms for modified Esc/Space/Tab),
  `is_slot_chord`, `is_text_key` (printable, modifiers ⊆ shift → goes through the IME path).
- `wheel_bytes`, `reports_mouse`, `WheelAccumulator::notches(delta_up, line_h)` (trunc toward 0), `clamp_cell`.
- `VtScrubber::{scrub, scrub_into}` — drops `CSI <|=|> … m`, holds split fragments ≤ 32 bytes.
- `TextInputGuard` (pure guard used by the session), `TextOrigin`.
- Links: `link_at(line, col, base)`, `trim_tail`, `resolve_link_path`, `local_path`;
  `follow_link(href, base) -> LinkFollow::{Nothing, OpenUrl, Reader(path), OpenFile(path), Missing(target)}`,
  `LinkFollow::banner()`, `url_failed_banner(href)`.
- Spawn plan: `Launch::{Shell, Command, Program}`, `SpawnPlan`, `TERM`; `GRACE` (3 s).

## Deviations from the Dart engine
- Copy trims trailing blanks per line even if the program wrote them (alacritty can't tell).
- OSC 8 spans are per cell (no 512 cap); the 6 xterm fixes are covered by `tests/vt.rs`.
