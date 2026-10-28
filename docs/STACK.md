# Stack

Spike status (2026-10-21): gpui + alacritty_terminal render a working, interactive `$SHELL`
in a window on this machine (X11 session, Zorin/mutter).

## Pinned versions

| Crate | Version | Notes |
|---|---|---|
| rustc / cargo | 1.93.0 | edition 2024 everywhere |
| `gpui` | 0.2.2 (crates.io) | default features (x11 + wayland + font-kit), blade/Vulkan renderer |
| `alacritty_terminal` | 0.26.0 | uses its own `tty` + `event_loop`, no second PTY crate |
| `async-channel` | 2 | same crate smol/gpui already pull in; terminal events to the UI |
| `libc` | `=0.2.189` | pinned on purpose, see gotchas |

Release profile: `opt-level=3`, `lto="fat"`, `codegen-units=1`, `panic="abort"`.
gpui only calls `catch_unwind` in its test harness (`src/test.rs`), so abort is safe for the app.
Dev profile: `opt-level=1` for workspace crates, `opt-level=2` for all deps (keeps the dev app smooth).

## Gotchas

- **libc 0.2.190 broke gpui on Linux.** `gpui -> gpui_http_client -> zed-async-tar -> xattr 0.2.3`
  uses `libc::ENOATTR`, removed for linux in libc 0.2.190. `rustix 1.1.5` (via alacritty) needs
  libc >= 0.2.182, so the window is 0.2.182..=0.2.189. Pinned in `[workspace.dependencies]` and
  pulled by `mx-app` so `cargo update` cannot float it. Revisit when gpui drops xattr 0.2.
- **No `unsafe` needed.** All crates, including `mx-app`, are `#![forbid(unsafe_code)]`, plus
  `unsafe_code = "forbid"` in `[workspace.lints]`. Because edition 2024 makes `env::set_var`
  unsafe, `tty::setup_env()` is not called; `TERM=xterm-256color` / `COLORTERM=truecolor` go
  into the child env map instead.
- **Key input on X11:** gpui derives modifiers from the X event `state`, but `key_char` from its
  own xkb state. Synthetic `XSendEvent` keys therefore get ctrl/alt right but ignore shift.
  Real keyboards are fine. There is no `InputHandler`/IME yet: text comes from `key_char`.
- **`Terminal` term lock and events:** alacritty calls `EventListener::send_event` while holding
  the term lock, so the proxy must never lock the term. `PtyWrite`, `TextAreaSizeRequest` and
  `ColorRequest` are answered from the PTY reader thread using a shared size + palette copy.
- **First paint** takes ~2 s on a cold debug start (naga/SPIR-V shader compile + font atlas).
- `cx.activate(true)` is a no-op on Linux (logged at info).
- The workspace builds a 560 MB debug binary (debuginfo); measure memory on release builds.

## Patched gpui (vendor/gpui)

`gpui` is gpui 0.2.2 copied from crates.io into `vendor/gpui` and wired with
`[patch.crates-io]` in the root `Cargo.toml` (`exclude = ["vendor"]` keeps it out of the workspace,
so it builds with the deps profile and without our lints). Three changes. First, in
`src/platform/linux/x11/client.rs` `X11Client::run`:

- **Bug:** the X11 loop is a calloop `Generic` source on the xcb fd, so it only wakes when the
  socket has new bytes. Any reply wait outside `process_x11_events` (runnables, window creation,
  randr queries) makes xcb read pending events into its own queue; the socket is then empty and
  those events (MapNotify, Expose, input) sit there until some unrelated X event arrives. On a quiet
  display the window stays black: MapNotify is never handled, so the refresh loop never starts.
  Repro: start the app on Xvfb and touch nothing; 10 s later the window is still one color with 0 CPU,
  and a single `xprop -set` on the window makes it paint.
- **Fix:** the `event_loop.run` callback (runs after each dispatch, right before calloop sleeps)
  calls `process_x11_events`, which drains `poll_for_event` until empty. Costs one non-blocking
  read per wake; no timer, idle CPU unchanged.
- Not fixed upstream: no gpui release after 0.2.2 (checked 2026-10-23) and Zed `main`
  (`crates/gpui_linux/src/linux/x11/client.rs`) has the same fd-only source. Drop the patch when
  a release fixes it.
- The e2e harness no longer needs `nudge`; `E2E_NUDGE=0` disables it.

Second change, in `src/elements/text.rs` `TextLayout::layout`:

- **Bug:** the measure closure owns the text runs and `truncate_line` shortens them in place. Taffy
  measures a text several times per frame (e.g. at width 0 inside an `Expanded`), so after a narrow
  pass the runs cover only the first few bytes and the rest of the line loses its weight/color
  (`ellipsis` texts showed "**pro**ducao…").
- **Fix:** each measurement truncates a clone of the runs.

Third change, frame cost of a streaming terminal (`src/scene.rs`, `src/bounds_tree.rs`, `src/window.rs`).
gpui rebuilds the whole scene every frame; with one terminal printing, the CPU went to inserting
primitives into the `BoundsTree`, sorting the scene and painting every glyph of the grid again.

- **Nested layers** (`Scene::push_layer`): a layer opened inside another reuses the outer order
  instead of inserting into the `BoundsTree`. Only the terminal nests layers (`ShapedLine::paint`
  opens one per line inside the grid's `paint_layer`); before, every text run of the grid was a tree
  insert against everything that overlaps it.
- **Replay of cached views** (`Scene::replay`, `BoundsTree::insert_span`): a range reused by
  `AnyView::cached` is inserted into the tree once, as its union bounds reserving as many orders
  as it spans, and its primitives keep their relative orders shifted by the new base. Before, every
  replayed primitive was inserted again. Falls back to the old path when replayed inside a layer.
- **Sprite sort** (`Scene::finish`): sprites use an unstable sort; equal `(order, tile)` keys are
  the same glyph at the same draw order, which never overlap.
- **`Window::invalidate_cached_view`**: lets a view re-render a cached child in the frame it is
  being rendered (a notify during draw only lands in the next frame). Used for pane headers and for
  terminals whose focus or typography changed.
- **`Window::record_paint` / `replay_paint`** (`PaintRecording`): keep the primitives painted for a
  region and insert them again moved to another origin, skipping shaping, raster bounds and atlas
  lookups. Refuses (returns false/None) on scale change, fractional device-pixel moves, paths,
  transformed sprites, or a region not fully inside the content mask. The terminal records each row.
- Not upstream. Drop when gpui gains retained rendering for unchanged subtrees.

## How the terminal works

`mx-term` (no gpui dependency):

- `TerminalSession::spawn(SpawnOptions, GridSize, TerminalPalette)` opens a PTY with
  `alacritty_terminal::tty::new`, wraps `Term` in `Arc<FairMutex<_>>` and starts alacritty's
  `EventLoop` thread (reads PTY, feeds the VT parser, writes queued input).
- Returns an `async_channel::Receiver<TermEvent>` (`Wakeup`, `Title`, `ResetTitle`, `Bell`,
  `ClipboardStore`, `Exited(code)`).
- `write`, `resize(GridSize)` (term + `TIOCSWINSZ` via `Msg::Resize`), `scroll`, `mode`, `child_pid`.
- `snapshot()` copies the visible viewport into an owned `Snapshot`: per cell char, resolved
  fg/bg `Rgb` (OSC 4/10/11 overrides > theme palette > xterm 256 cube), bold/italic/underline/
  strikeout, wide/spacer flags, and the cursor. Inverse/hidden/dim are applied there, so the UI
  never sees alacritty types.
- `encode_key(key, key_char, Mods, app_cursor)` maps gpui-style key names to bytes
  (ctrl/alt/shift combos, arrows with DECCKM, F1-F12, xterm modifier codes). Ported from Zed's
  `terminal/src/mappings/keys.rs`.

`mx-app` (binary `maestria`):

- Embeds the four Hack faces with `include_bytes!` and registers them via `text_system().add_fonts`.
- `TerminalView` (entity): owns the session, a `FocusHandle`, and a `spawn_in` task that awaits
  the event channel, drains bursts, and calls `cx.notify()` once per burst. `Exited` quits the app.
  Handles `on_key_down` (ctrl+shift+v pastes, bracketed when enabled), mouse wheel scrollback,
  click-to-focus.
- `TerminalElement` (custom `Element`): in `prepaint` it measures the cell (`advance('m')` of Hack
  14 px, line height `round(14*1.3)`), derives cols/rows from its bounds, calls `session.resize`
  (no-op when unchanged, so window resize resizes the PTY), takes a snapshot and builds:
  merged background quads per row-run of equal bg, text batches per run of equal style shaped
  with `shape_line(..., force_width = cell_width)` (same trick as Zed's `BatchedTextRun`), and the
  cursor (block = inverted cell when focused, beam/underline bars, hollow box when unfocused).
  `paint` draws bg, rects, lines, cursor.

## Verified

- `cargo test -p mx-term -p mx-theme`: key encoding, session spawn/exit code, bold SGR, echo, resize.
- `cargo clippy -p mx-term -p mx-app --all-targets`: clean.
- Ran the binary: zsh child spawned (`spawned shell pid ...`), prompt + right prompt rendered
  in Hack, typed commands executed, Up recalls history, ctrl+l clears, ctrl+c interrupts,
  `ls --color` shows bold + colored + colored-background cells, X11 resize to 700x400 gave
  `stty size` = `22 83`, `exit` quit the app cleanly with code 0.

## Measured

Debug build (`opt-level` 1/2, full debuginfo), idle after ~70 s with one zsh session at 960x600:

- `VmRSS` 102 056 kB (`RssAnon` 25 216 kB, `RssFile` 76 824 kB, mostly the mapped debug binary)
- 31 threads, ~0.2 % CPU idle (2 ticks in 10 s)

Release numbers still to measure (fat LTO build was skipped while the machine was loaded).
