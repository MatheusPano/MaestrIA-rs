# 06 — Delta 2.4.0 → 2.7.0 (behavioural spec)

Specs 01–05 describe Flutter MaestrIA **v2.4.0**. The version users run is **v2.7.0**. Its source is `origin/main` in `/home/lucas/maestria/repo`, 17 commits ahead and 101 files changed. This document covers only what changed between those two versions.

How it was read: from `git -C /home/lucas/maestria/repo show <commit>`, `git log -p origin/main -- <path>` and `git show origin/main:<path>`. Read-only; the local checkout was never touched.

Conventions:
- UI strings are verbatim Portuguese in backticks.
- "Rust landing" names the crate, module and file each item lands in.
- "Already in Rust" lists what the Rust code already has and how it differs from Dart.
- Tests are language-neutral given/when/then cases taken from the added or changed Dart tests.
- Part 2 assigns files to work packages. Where a section's own "Rust landing" disagrees with it, **Part 2 wins**.

## Index — commit → section

| Version | Commit | ID | Feature | Section |
|---|---|---|---|---|
| 2.5.0 | 4cafe46 | TASK#80537 | App auto-update from GitHub releases (status-bar chip, confirm dialog) | A |
| 2.5.0 | 140580a | TASK#80544 | Release workflow from the Actions form (`versao` input, SHA256SUMS) | A |
| 2.5.0 | e5101c7 | TASK#80567 | Rounded inset hover (`MxHover`) on rows and menus | E |
| 2.6.0 | e9c30be | — | Plugin catalog + per-plugin auto-update | B |
| 2.6.0 | d77be94 | — | Catalog auto-update toggle; app `autoUpdate` toggle (Settings → `Atualizações`); sticky selection on scroll; new app icon / branding | A, B, C |
| 2.6.1 | fcc695b, e570922 | BUG#80709 | Join rows Claude wrapped; copy with reflow in Claude tabs (⌘C and `Copiar`) | C |
| 2.7.0 | 5c7af80 | TASK#82250 | Per-panel hotkeys (replace ⌘1–9 slots); drag panes by header; session cards on empty screen | E |
| 2.7.0 | 28eefb2 | TASK#82251 | Find in pane terminal (⌘F; sidebar search moves to ⇧⌘F) | C |
| 2.7.0 | c707872 | TASK#82252 | Attach background sessions from history (`claude attach <id>`) | E |
| 2.7.0 | a61b94e | TASK#82253 | New plugin widgets: CodeEditor, TextInput, Markdown, Selectable/SelectionLines, Sprite, ZoomView, Anchor + modal grow animation, list block additions, 8 capability flags | D |
| 2.7.0 | 610086e | TASK#82249 | Compact switch (`MxSwitch`) + settings tweaks; remove dirty count / header subtitle / rail badge | E |
| 2.7.0 | b043ae0 | TASK#82254 | Sessions API for plugins (sendKeys, pasteClipboard, rename, wake, hibernate, portrait, background openClaude, breaking `session.close`), `sessions.close` permission, new events, PreCompact/PostCompact hooks, notifications with portrait, `folders` setting type, plugin kind tag | F |
| 2.7.0 | 281c232 | TASK#82254 | `folders` setting in a picker modal; setting descriptions as `?` tooltip | E |
| — | fa6d0c1, be63dcb, 1587aa2, 9556243 | — | Version bumps only | A |

## Decisions for the user

1. **Auto-update target (blocking for WP1).** The Rust app must update to the **maestria-rs `.deb`**. It must not install the Flutter `maestria_*_amd64.deb` over itself.
   - Copying Dart literally breaks one of two ways. The default cargo-deb variant is also package `maestria` at `/opt/maestria`, so it would replace itself with Flutter. The `maestria-rs` variant would never update: it fails the path check, and 3.0.0 is above 2.7.0.
   - Option 1 (recommended): a separate GitHub repo for maestria-rs, with `maestria-rs_*` assets.
   - Option 2: the same repo with an `rs-v*` tag namespace, read from the releases list instead of `releases/latest`.
   - Option 3: Rust officially replaces Flutter as package `maestria`.
   - Whichever option is chosen, whether the updater is enabled should come from a build-time release flag, not from `CARGO_PKG_VERSION`. Details in A, "DECISION FOR USER".
2. **macOS native notifications with portrait attachment (F).** Dart uses `UNUserNotificationCenter`, which needs a signed bundle. Proposal: in Rust, macOS always uses the osascript fallback; Linux uses `notify-send -a maestria -i <img>`.
3. **Plugin catalog and the `maestria` minimum version (B).**
   - Proposal: Rust reuses the official catalog unchanged (`MatheusPano/maestria-plugins`, release `catalog`).
   - Because Rust reports 3.0.0, it accepts every entry's `maestria` minimum. The message `Pede uma MaestrIA mais nova` never fires.
   - That is only correct if Rust implements every 2.7 capability flag. Option: report the protocol level as 2.7.0 for the comparison until it does.
4. **Two different `autoUpdate` keys:**
   - Top-level `autoUpdate` is the app updater. Default false; written only when true.
   - `plugins.autoUpdate` is the catalog updater. Default true; written only when false.
   - Implementers must not merge them.
5. **Not ported on purpose:**
   - The temporary `_log()` DEBUG in `sticky_selection.dart` at origin/main.
   - The DEBUG lines in `pty.dart`'s `wheel()`, which 610086e already removed.

## Existing Rust code that is already wrong for 2.7 (fix inside the owning WP)

- **CodeEditor data loss** (`crates/mx-app/src/rfw_view/code_pane.rs`, `CodePane::apply`): it reloads the buffer when a doc reappears with the same `revision` but different `text`. Dart never does this. Dart test ed.2 (doc a → b → a) would lose what the user typed. Owner: WP3. Every other CodeEditor discrepancy is listed in D §TASK#82253.1.
- **Keymap** (`crates/mx-app/src/keymap/action.rs` ~L179/L204):
  - `Search` is still bound to ⌘F / Ctrl+Shift+F. 2.7 gives ⌘F (Linux Ctrl+Shift+F) to `find` and moves `search` to ⇧⌘F (Linux Ctrl+Alt+F).
  - The ⌘1–9 / Alt+1–9 slot system still exists: `Chord::slot`, `slot_of`, `Hit::Slot`, `select_index` and their tests. 2.7 removed it. Owner: WP5.
- **Rail "+"** (`crates/mx-app/src/rail/mod.rs:120-123`) still opens the URL install dialog. In 2.7 it opens the catalog. Owner: WP5, which calls into the catalog dialog from WP1.
- **`initialize` capability flags:** Rust sends only `editor`. 2.7 sends 16 more:
  - D: `sprites`, `zoom`, `textInput`, `markdown`, `arrows`, `modalFrom`, `selectable`.
  - F: `openInBackground`, `pasteImage`, `keys`, `wake`, `hibernate`, `portrait`, `closeSessions`, `rename`.
  - A flag must only be advertised once its feature has landed. Owner of the flag list: WP6.
- **Pane drag:** Rust allows dragging with a single pane, and its ghost has different geometry. 2.7 requires 2 or more panes and anchors the ghost at (18,17). Owner: WP5.
- `release.yml` in Rust is still the pre-TASK#80544 shape. The icons in `crates/mx-app/assets/icons` are the old ones. Owner: WP1.

---

# Part 1 — Behaviour by area


## A — App auto-update, release pipeline, packaging, branding

Sources (Flutter, origin/main): `lib/services/updater.dart`, `lib/ui/update_chip.dart`,
`lib/ui/settings.dart` (`_Updates`), `lib/services/store.dart`, `lib/ui/notices.dart`,
`lib/main.dart`, `.github/workflows/release.yml`, `packaging/**`, `branding/**`,
`pubspec.yaml`, `test/updater_test.dart`. Commits 4cafe46 (TASK#80537), 140580a
(TASK#80544), d77be94 (toggle + icon), fa6d0c1/be63dcb/1587aa2/9556243 (version bumps only).

---

### TASK#80537 — Auto-update from GitHub releases

#### Behaviour

**Build version.** `mxVersion` = compile-time `MAESTRIA_VERSION` (`--dart-define`), set by
`make-deb.sh` / `make-dmg.sh` to the pubspec `X.Y.Z` (no revision). Empty (dev build) ⇒
updater disabled entirely.

**`enabled`** (the updater can act at all):
- `current` empty → false.
- macOS: true iff the running executable path contains `.app/Contents/MacOS/` (bundle =
  prefix up to and including `.app`; any location, not only `/Applications`).
- Linux: true iff `resolvedExecutable` starts with `/opt/maestria/` AND platform is x64
  (release ships only an amd64 `.deb`).
- Other OS → false.

**`active`** (user toggle, see d77be94 below). Default **false**.

**Stages** (`UpdateStage`): `idle` → `fetching` → `ready` → (Linux) `installing` →
`installed`. Errors in `fetching` drop back to `idle` silently (only `debugPrint`); retried
next tick. No toast/banner for background failures.

**Schedule.**
- `start()` (called at end of `store.init()`, after `_loadConfig`, so `active` is already
  loaded): if `!enabled` return; delete `$STATE/update/` (leftovers: either the update
  already took effect or a half-finished attempt); if `active`, schedule first check after
  **30 s** (`firstDelay`), then `Timer.periodic` every **6 h** (`every`).
- `setActive(on)`: equal → no-op. Cancels timers. If `!enabled` stop (value still stored).
  `on` → schedule with delay **0** (check immediately, then every 6 h). `off` while
  `stage==ready` → discard: clear staged path, release, relaunch flag, **ETag** (otherwise
  re-enabling gets a 304 for the discarded release and never re-downloads), stage idle,
  delete `$STATE/update/`. Installed-on-disk (Linux `installed`) is kept. notify.

**`check()`**: guard `enabled && active && stage==idle`. Query latest; if null or
`compareVersions(latest.version, current) <= 0` return. `stage=fetching`; download/verify;
if toggle was switched off during download → discard; else `release=latest`,
`stage=ready`. On exception: clear staged, if fetching → idle.

**Latest-release request.**
- `GET https://api.github.com/repos/MatheusPano/MaestrIA/releases/latest`
- Headers: `User-Agent: maestria/<current>`, `Accept: application/vnd.github+json`,
  `If-None-Match: <etag>` when an ETag was stored. Anonymous (no token; 60 req/h/IP limit;
  304s do not count).
- 304 → null (no news). Non-200 → error `releases/latest: HTTP <code>`. 200 → store
  response `etag` header, parse.

**Release parsing** `MxRelease.fromJson(json, suffix)`:
- `tag_name` missing → null. `version` = tag with leading `v` stripped.
- Iterate `assets[]` (skip non-objects): `name == "SHA256SUMS"` → `sumsUrl =
  browser_download_url`; `name.endsWith(suffix)` → `asset=name`, `assetUrl=url` (last match
  wins). No matching asset/url → null. `sumsUrl` may be null.
- `suffix`: macOS `_macos.dmg`, Linux `_amd64.deb`. Asset names are
  `maestria_X.Y.Z-R_amd64.deb` / `maestria_X.Y.Z-R_macos.dmg`.

**`compareVersions(a,b)`**: strip leading `v`; split on regex `[.+-]`; each part
`int.tryParse` else 0; compare element-wise padding missing with 0; return sign (-1/0/1).
So `2.10.0 > 2.9.1`, `2.5 == 2.5.0`, `v3 > 2.99.99`; note `2.5.0-1` is compared as
`[2,5,0,1]`.

**`expectedSum(sums, file)`**: for each trimmed line match
`^([0-9a-fA-F]{64})\s+\*?(.+)$`; if group2 trimmed == file exactly → group1 lowercased.
Else null.

**Fetch & verify** (`_fetch`):
1. `sumsUrl == null` → error `<version> sem SHA256SUMS` (old releases are never installed).
2. macOS: run `test -w <parent dir of .app>`; failure → error (don't offer what can't be
   swapped on quit).
3. Recreate `$STATE/update/` (delete recursively, create).
4. Download SHA256SUMS → `$STATE/update/SHA256SUMS`; `expectedSum(..., asset)` null → error
   `<asset> não está no SHA256SUMS`.
5. Download asset → `$STATE/update/<asset>` (follow redirects; non-200 → `<url>: HTTP n`;
   same User-Agent).
6. sha256: macOS `shasum -a 256 <f>`, Linux `sha256sum <f>`; first whitespace token,
   lowercased; mismatch → `<asset>: sha256 <got>, esperado <want>`.
7. Linux: staged = the `.deb` path. macOS: `hdiutil attach -nobrowse -readonly -mountpoint
   $STATE/update/mnt <dmg>`; `ditto $mnt/maestria.app $STATE/update/maestria.app`; always
   `hdiutil detach $mnt -force`; delete dmg; `codesign --verify --deep <app>` (failure →
   error); `xattr -dr com.apple.quarantine <app>` (exit ignored). staged = app path.

**Linux install** (`install()`, only from chip confirm): requires `stage==ready` and staged.
`stage=installing`; run `pkexec apt-get install -y <staged.deb>`.
- exit 0 → `stage=installed`, return null.
- else `stage=ready`; exit **126** (password dialog cancelled) → `Instalação cancelada`;
  otherwise `Não deu pra instalar (<last line of stderr>). No terminal: sudo apt install <staged>`.
  Caller shows it via `store.showBanner(error, sticky: true)` and aborts.

**Quit hook** (`onQuit`, awaited in `didRequestAppExit` *after* all sessions are shut
down, right before returning `exit`):
- mac condition: macOS && `stage==ready` && staged. linux condition: Linux &&
  `stage==installed` && relaunch requested. Neither → nothing.
- Spawn detached `/bin/sh -c <script> maestria-update <pid> <target> <staged or ""> <"1" if
  relaunch else "">` (paths as positional args, never interpolated). target = bundle path
  (mac) or resolved executable (linux).
- mac swap script (verbatim):
  ```sh
  while kill -0 "$1" 2>/dev/null; do sleep 0.2; done
  old="$2.old"
  rm -rf "$old"
  if mv "$2" "$old"; then
    if ditto "$3" "$2"; then
      rm -rf "$old" "$(dirname "$3")"
    else
      rm -rf "$2"; mv "$old" "$2"
    fi
  fi
  if [ -n "$4" ]; then open "$2"; fi
  ```
  (Arg numbering: `$0`=`maestria-update`, `$1` pid, `$2` installed app, `$3` new app, `$4` relaunch.)
- linux relaunch script:
  ```sh
  while kill -0 "$1" 2>/dev/null; do sleep 0.2; done
  setsid "$2" >/dev/null 2>&1 < /dev/null &
  ```
- So: macOS installs on every quit once ready (no prompt); Linux only installs on click
  and only relaunches when the click asked for it.

**`installsOnQuit`** = macOS.

**Store wiring.** `AppStore.updater` field; `init()` adds `updater.addListener(notifyListeners)`
and `updater.start()`; `dispose()` disposes it.

**Status bar chip** (`UpdateChip`, inserted in `StatusBar` row immediately **left of**
`NoticeBell`):
- Hidden unless `release != null` and stage is `ready` or `installing`.
- Pill: height 20, right margin 4, h-padding 6, radius 4; bg `Mx.bgHover` on hover else
  transparent. Icon `arrow_circle_up_rounded` 14 + gap 4 + text 11 / height 1 / w700.
  Colour `Mx.fg` on hover else `Mx.accent`.
- Text: `instalando…` while installing, else the version (e.g. `2.5.0`).
- Tooltip (wait 400 ms): installing → `Instalando a {v}…`; macOS → `maestria {v} pronta ·
  entra quando você fechar o app`; Linux → `maestria {v} pronta pra instalar`.
- Cursor click (basic while installing); tap disabled while installing; tap →
  `confirmUpdate`.

**Confirm dialog** (`confirmUpdate`): `live` = Claude tabs not exited.
- Title `Atualizar pra {v}?` (15 px); bg `Mx.bgSidebar`; content width 440; lines = icon 16
  `Mx.fgDim` + 10 gap + text 13, v-pad 4:
  - `restart_alt_rounded`: live==0 → `O app fecha e abre de novo, já na {v}`; else `O app
    fecha e abre de novo: as {live} sessões abertas são encerradas, e os painéis voltam
    como estavam`.
  - macOS: `schedule_rounded` `Sem pressa: ela entra sozinha quando você fechar o app`;
    Linux: `lock_outline_rounded` `O sistema vai pedir a sua senha pra instalar o pacote`.
- Actions: `Depois` (text, false) · `Reiniciar agora` (filled, true).
- On true: Linux → `install()`; error → sticky banner, stop. Then `relaunchAfterQuit()`
  and trigger the normal app exit path (same as ⌘Q, cancelable), which shuts sessions and
  calls `onQuit`.

**Release notes** (`packaging/NOTAS-DA-RELEASE.md`, new section before `## Primeiro uso`,
final text after d77be94):
```
## Atualizações

O app pode se atualizar sozinho, se você ligar em **Configurações →
Atualizações** (vem desligado). Ligado, quando sai versão nova ele baixa em
segundo plano e mostra o número dela na barra de baixo, ao lado do sino. No Mac
ela entra quando você fecha o app; no Linux, quando você clica (o sistema pede a
senha pra instalar o pacote). Nada reinicia sem você pedir.
```

**Packaging**: `make-deb.sh` / `make-dmg.sh` pass `--dart-define=MAESTRIA_VERSION="$VERSION"`.

#### config.json keys
None in this commit (toggle added in d77be94). On disk: `$STATE/update/` scratch dir
(SHA256SUMS, asset, `mnt/`, `maestria.app`), wiped on `start()`.

#### Shortcuts
None.

#### Tests (`test/updater_test.dart`)
- Version compare is numeric: given `2.10.0` vs `2.9.1` → 1; `2.4.0` vs `2.4.1` → -1.
- Ignores tag `v` and missing trailing parts: `v2.5.0`==`2.5.0` → 0; `2.5`==`2.5.0` → 0;
  `v3` vs `2.99.99` → 1.
- SHA256SUMS lookup: given sums `"<a×64>  maestria_2.5.0-1_amd64.deb\n<B×64> *maestria_2.5.0-1_macos.dmg\n"`:
  when looking up the `.deb` → `a…`; the `.dmg` (binary-mode `*`) → `b…` lowercased.
- Not found: `maestria_2.5.0-1_amd64.deb.zip` → null; empty sums, `x` → null.
- Release JSON `{tag_name:"v2.5.0", assets:[deb→https://x/deb, dmg→https://x/dmg, SHA256SUMS→https://x/sums]}`
  with suffix `_macos.dmg` → version `2.5.0`, asset `maestria_2.5.0-1_macos.dmg`, assetUrl
  `https://x/dmg`, sumsUrl `https://x/sums`.
- Suffix `_arm64.deb` (no matching asset) → null.
- Same JSON without SHA256SUMS, suffix `_amd64.deb` → release exists, `sumsUrl` null.
- Updater with `current=""` → `enabled == false`.

#### Rust landing
- **mx-sys** `src/updater/` (new): `version.rs` (`compare_versions`), `sums.rs`
  (`expected_sum`), `release.rs` (`Release::from_json(json, suffix)`), `fetch.rs`
  (GitHub request with ETag via `reqwest`, download, sha256 — prefer in-process `sha2`
  over shelling out), `install.rs` (pkexec apt-get, exit-126 mapping, detached relaunch
  script; macOS dmg swap behind `cfg(target_os="macos")`), `mod.rs` (`Updater` state
  machine + `Stage`, pure/time-injectable so tests don't need timers). Add `reqwest`,
  `sha2` to `crates/mx-sys/Cargo.toml` (reqwest already in workspace deps, used by
  mx-claude). Tests: `crates/mx-sys/tests/updater.rs` (port the 8 cases).
- **mx-store** `src/updates.rs` (new): owns the `Updater`, start-after-load, 6 h timer via
  existing `timers.rs` pattern, `set_auto_update`, `install`, `relaunch_after_quit`,
  `on_quit`; exposes stage/version for UI. Hook `on_quit` into `Store::shutdown` *after*
  session shutdown (`crates/mx-store/src/lifecycle.rs` or `store.rs` — single call site).
- **mx-app**
  - `src/status_bar/update_chip.rs` (new) + one-line insertion before the bell in
    `src/status_bar/mod.rs`.
  - `src/chrome/dialog/` or `src/app/confirms.rs`: confirm dialog `Atualizar pra {v}?`
    (prefer new `src/app/update_confirm.rs` to avoid conflicts with confirms.rs owners).
  - App-exit trigger: reuse the quit path in `src/app/mod.rs` (`on_app_quit` already runs
  `store.shutdown()` with a 3 s grace; `on_quit` must run after that future resolves).
- **Version constant**: use `env!("CARGO_PKG_VERSION")` of mx-app, but gate "enabled" on an
  explicit build-time flag (e.g. `option_env!("MAESTRIA_RELEASE")` set by
  `packaging/linux/make-deb.sh` / `make-dmg.sh`) so `cargo run` builds never self-update —
  Cargo always has a version, unlike the Dart define.

#### Already in Rust
Nothing implemented (no updater, no chip, no `Atualizações` section). `reqwest` only in
mx-claude. Relevant existing facts that change the design:
- Rust package has two cargo-deb variants in `crates/mx-app/Cargo.toml`: default
  `maestria` (installs `/opt/maestria/maestria`, same package name/path as Flutter →
  replaces it) and `rs` variant `maestria-rs` (`/opt/maestria-rs/maestria-rs`,
  `/usr/bin/maestria-rs`, app id `br.com.marrow.maestria-rs`; latest commit 271ec25).
  `packaging/linux/make-deb.sh` builds the default variant only.
- State dir defaults to `~/.maestria-rs` (commit 59d0083), so `$STATE/update/` and the
  `autoUpdate` key are naturally separate from the Flutter install.
- Workspace version is `3.0.0`; Flutter releases are `2.x`. Against the Flutter repo
  `compareVersions(2.7.0, 3.0.0) < 0` ⇒ the Rust app would never see an update; and the
  Flutter `_amd64.deb` suffix would match the Flutter asset.
- `Makefile` `tag` target and `.github/workflows/release.yml` still print/point to
  `https://github.com/MatheusPano/MaestrIA/releases` and the old `inputs.tag` workflow.
  The repo has no git remote configured.

#### DECISION FOR USER — what does the Rust app update itself to?
The Flutter updater hard-codes repo `MatheusPano/MaestrIA`, suffix `_amd64.deb`, and the
`/opt/maestria/` gate. Ported verbatim, a Rust build would download the **Flutter** `.deb`
and `apt-get install` it over itself (default variant shares package name `maestria`), or
do nothing (rs variant fails the `/opt/maestria/` gate; 3.0.0 > 2.7.0). It must target the
maestria-rs package. Options:
1. **Own GitHub repo for maestria-rs** (e.g. `<org>/maestria-rs`) with assets
   `maestria-rs_X.Y.Z-R_amd64.deb` + `SHA256SUMS`; updater constants: repo = that, suffix
   `_amd64.deb` with prefix check `maestria-rs_`, gate `/opt/maestria-rs/`. Cleanest;
   needs a repo + CI. Recommended.
2. **Same repo `MatheusPano/MaestrIA`, distinct tag namespace** (`rs-v3.0.0`) — but
   `releases/latest` returns only one release, so the updater must instead list
   `GET /repos/{repo}/releases?per_page=20` and pick the newest non-draft, non-prerelease
   whose tag starts with `rs-v` and which has a `maestria-rs_*_amd64.deb`. Mixes the two
   products' release pages.
3. **Rust replaces Flutter as package `maestria`** (default variant, 3.x > 2.x in apt):
   updater can then reuse `MatheusPano/MaestrIA` once that repo publishes the Rust build —
   only valid if the team agrees the Rust app *is* the next maestria. Until then, ship
   the updater disabled.
Whatever is chosen: the asset match must include the package-name prefix (not only the
suffix), the install gate must match the installed variant's path (`/opt/maestria-rs/` for
the rs variant), the pkexec command installs that file (`apt-get install -y <path>`
works for either), and the User-Agent should identify `maestria-rs/<ver>`.

---

### d77be94 (app part) — Auto-update toggle in Settings

#### Behaviour
- New settings section **`Atualizações`** (icon `system_update_alt_rounded`), placed
  between `Sessões` and `Conta & uso`. Final order: `Aparência`, `Atalhos`, `Programas`,
  `Plugins`, `Sessões`, `Atualizações`, `Conta & uso`.
- Section content (`_Updates`):
  - Heading `Atualizações`, hint: dev build (`current` empty) → `Build de desenvolvimento:
    sem número de versão, não há com o que comparar a release publicada.`; else `Esta é a
    {current}.`
  - Toggle (`_Toggle`, the compact switch row from TASK#82249) value `updater.active`;
    `onChanged` = `store.setAutoUpdate` only if `updater.enabled`, else null (rendered
    disabled). Title `Atualizar sozinho`. Subtitle: enabled → `Quando sai versão nova, ela
    é baixada em segundo plano e o número aparece na barra de status. {when} Nada reinicia
    sem você pedir.` with `{when}` = macOS `Entra quando você fecha o app.` / Linux
    `Instala quando você clica nele (o sistema pede a senha).`; not enabled → `Só vale pro
    app instalado pelo pacote da release.`
- `store.setAutoUpdate(on)`: equal → no-op; `updater.setActive(on)`; `_save()`; notify.
- The same commit's plugin auto-update switch (`Atualizar os plugins sozinho`) lives in the
  Plugins section and belongs to the plugin-catalog section of this delta, not here.
- Sidebar rail `+` button now opens `showPluginCatalog` instead of `showInstallPlugin`
  (catalog section).

#### config.json keys
| key | type | default | written | read |
|---|---|---|---|---|
| `autoUpdate` | bool | `false` | only when true (`if (updater.active) 'autoUpdate': true`), between `hibernateMinutes` and `theme` | `j['autoUpdate'] as bool? ?? false`, read right after `hibernateMinutes`, before `plugins.load` |
Back-compat: absent in 2.4 configs ⇒ false; older apps ignore the key.

#### Shortcuts
None.

#### Tests
No Dart test for the toggle. Language-neutral cases to add:
- Given config without `autoUpdate` → active false; given `{"autoUpdate":true}` → active true.
- Given active false → saved config has no `autoUpdate` key; active true → `"autoUpdate":true`.
- Given stage ready, when set_active(false) → stage idle, release none, ETag cleared, update dir removed.
- Given enabled and active false, when set_active(true) → a check is scheduled with zero delay.
- Given not enabled, when set_active(true) → value persisted, no check scheduled, UI switch disabled.
- Given toggle switched off during fetching → result discarded (stage idle, no release).

#### Rust landing
- **mx-core** `src/persist/config.rs` + `src/persist/read.rs`: `auto_update: bool` field,
  read/write rule above; test in `crates/mx-core/tests/persist.rs`.
- **mx-store** `src/prefs.rs`: `auto_update` in `Prefs`; setter delegates to `updates.rs`.
- **mx-app** `src/chrome/settings/model.rs`: add `Section::Updates` (`"Atualizações"`) to
  `ALL` between Sessions and Account; new `src/chrome/settings/updates.rs` for the section
  body; register in `settings/mod.rs` + `dialog.rs` match arm.

#### Already in Rust
`Section` enum has 6 sections, no Updates. `Prefs` has no auto-update field.

---

### TASK#80544 — Release workflow (`.github/workflows/release.yml`)

#### Behaviour
- `workflow_dispatch` input renamed `tag` → **`versao`**, description `a versão (ex.:
  2.5.0) -- nova é criada, existente é reempacotada`, required.
- Removed global `env.TAG`; job `versao` now has output `tag` from step `id: tag`:
  - Non-dispatch (tag push): `tag=$GITHUB_REF_NAME`.
  - Dispatch: `v=${VERSAO#v}`; must match `^[0-9]+\.[0-9]+\.[0-9]+$` else
    `::error::'$VERSAO' não é uma versão (ex.: 2.5.0).`; `tag=v$v`.
  - If `git ls-remote --exit-code --tags origin refs/tags/$tag` succeeds → `$tag já existe:
    reempacotando.` and continue (rebuild only).
  - New tag requires `GITHUB_REF == refs/heads/main` else `::error::versão nova sai da
    main, e este run é de $GITHUB_REF_NAME.`
  - `make bump V="$v"`; git identity `github-actions[bot]` /
    `41898282+github-actions[bot]@users.noreply.github.com`; `git diff --quiet || git
    commit -am "release $tag"`; `git tag "$tag"`; `git push --atomic origin HEAD:main
    "$tag"`. (Build runs in the same workflow run because tags pushed with `GITHUB_TOKEN`
    don't trigger workflows.)
  - Second checkout at `ref: steps.tag.outputs.tag`, then the existing "tag matches
    version file" check with `TAG` from the step output.
- `deb`, `dmg`, `publicar` checkout `needs.versao.outputs.tag`; `publicar` needs
  `[versao, deb, dmg]`, `TAG` env from the output.
- `publicar` now generates checksums before upload:
  `(cd anexos && sha256sum *.dmg *.deb > SHA256SUMS && cat SHA256SUMS)`; SHA256SUMS is
  uploaded with the other assets (`--clobber` on re-runs). The updater refuses releases
  without it.
- Header comment documents: Actions → "Run workflow" on main with the version; manual path
  `make bump V=… && git commit -am … && git push; git tag vX && git push origin vX` still works.
- The `release vX.Y.Z` commits (2.5.0, 2.6.0, 2.6.1, 2.7.0) are this bot commit: only
  `pubspec.yaml` `version:`.

#### Rust landing
- `.github/workflows/release.yml` (maestria-rs): same `versao` input/step logic, version
  file = `Cargo.toml` `[workspace.package] version` via `make bump V=`; add the
  `sha256sum *.dmg *.deb > SHA256SUMS` step (assets must carry the chosen
  `maestria-rs_` prefix, see decision). `Makefile` `tag` target URL must point at the
  chosen repo.

#### Already in Rust
`release.yml` exists but is the **pre-80544** shape (`inputs.tag`, global `env.TAG`, no
bump/commit/tag step, no SHA256SUMS). Deb job builds via `build-deb.sh` (default variant
`maestria`, not `maestria-rs`).

#### Tests
None (CI).

---

### Branding / new app icon (d77be94)

#### Behaviour
- New `branding/` folder: `app-icon.svg` (macOS, 824 square with Apple margin + shadow),
  `app-icon-windows.svg` (no margin/shadow → `windows/runner/resources/app_icon.ico`),
  `mark.svg`/`mark-light.svg` (the "M regente" alone), `logo.svg`/`logo-light.svg` (mark +
  "MaestrIA" in Geist 500 "Maestr" / 600 "IA" as outlines), PNG renders, `build.py`
  (needs `resvg`, `magick`, Python `fonttools`, `uharfbuzz`, `Geist[wght].ttf`; usage
  `python build.py <Geist.ttf> <out>` → `out/branding/` + `out/png/`), `README.md`.
- Accent gradient `#7AA2F7` → `#C3A6FF` (Dark theme blue/purple), glow `#C3A6FF` α .55→0,
  icon bg gradient `#20242C`→`#0D0F13`, pane tiles `#2C313A` α .8; `-light` variants
  use the accent blue tip instead of white+glow.
- `macos/Runner/Assets.xcassets/AppIcon.appiconset/app_icon_{16,32,64,128,256,512,1024}.png`
  replaced; Linux `.deb` picks these up (make-deb.sh copies `app_icon_${size}.png` into
  hicolor). Windows `.ico` replaced.

#### Rust landing
- Copy `branding/*.svg` into maestria-rs `branding/` and regenerate
  `crates/mx-app/assets/icons/{32,64,128,256,512,1024}.png` from the new
  `app_icon_*.png` (or `git show origin/main:macos/Runner/Assets.xcassets/AppIcon.appiconset/app_icon_N.png`).
  Asset-only work package, no code.

#### Already in Rust
`crates/mx-app/assets/icons/*.png` are the old (2.4) icon.

---

### Misc in this area
- `pubspec.yaml`: version `2.4.0+1` → `2.7.0+1`; new deps `re_editor ^0.10.0`,
  `re_highlight ^0.0.3` (CodeEditor widget — covered by the TASK#82253 section).
- `lib/main.dart`: only the `await store.updater.onQuit()` after session shutdown in
  `didRequestAppExit`.
- `macos/Runner/MainFlutterWindow.swift`: `NotificationRelay` (UNUserNotificationCenter,
  `notify` method channel, `notificationTapped` callback) — belongs to TASK#82254
  notifications section, not this one.
- `.gitignore`: adds `.claude/worktrees/` and `/docs/claude-desktop-mcps.md` (irrelevant to Rust behaviour).


## B — Plugin catalog and per-plugin auto-update (2.6.0: e9c30be, catalog parts of d77be94)

Sources: `lib/services/plugin_catalog.dart` (new), `lib/services/plugins.dart` (`PluginKind`, `PluginOrigin`, `autoUpdate`/`manualUpdate`/`origins`, `stage(..., origin, ref)`), `lib/services/store.dart` (`pluginUpdates`, `setPluginAutoUpdate`, `setPluginManualUpdate`), `lib/ui/plugin_dialogs.dart` (`showPluginCatalog`, `PluginKindTag`, `confirmStagedPlugin`, `updatePlugin`), `lib/ui/settings.dart` (`_AutoUpdate`, plugin row/menu), `lib/ui/sidebar_rail.dart`, `docs/plugins.md`, `test/plugin_catalog_test.dart`, `test/plugins_test.dart`.

Out of scope here (other sections): the app's own updater (`updater.dart`, top-level `autoUpdate` key — note the name clash, see B.3), the `folders` setting type and its picker (TASK#82254), new capability flags and `sessions.close` permission (TASK#82253/82254), `MxSwitch`/`_Toggle` look (TASK#82249).

### B.1 — Catalog model and source

**Official catalog URL** (constant `mxOfficialCatalog`):
`https://github.com/MatheusPano/maestria-plugins/releases/download/catalog/catalog.json`
(a release asset on tag `catalog` of the `maestria-plugins` repo; GitHub answers with a redirect to object storage — the HTTP client must follow redirects). `PluginUpdates.catalogs` is a list (default `[official]`); the UI only ever shows the official one. No user-configurable catalog list in 2.7.

**Fetch**: plain `GET`, header `User-Agent: maestria`, body decoded as UTF-8; status != 200 → error `HTTP <code>`. No caching on disk; the last parsed catalog per URL is kept in memory (`_loaded[url]`) and shown while a refresh is running.

**Format** (`catalog.json`):
```json
{ "schema": 1,
  "plugins": [
    { "id": "maestria.git", "name": "Git", "version": "1.2.1", "maestria": 1,
      "description": "…", "author": "…", "permissions": ["sessions.create"],
      "kinds": ["feature"], "icon": "https://…/git.svg",
      "url": "https://github.com/…/releases/download/git-v1.2.1/git-1.2.1.zip",
      "sha256": "2f2c3c8f84…" },
    { "id": "com.lucas.foo", "version": "0.3.0", "git": "https://github.com/lucas/maestria-foo", "ref": "main" } ] }
```
Parse (`PluginCatalog.parse`): top level must be an object with `plugins` array, else `FormatException('não é um catálogo')`. `schema` is not read. Each entry parsed independently; a bad entry is dropped, never fails the catalog. Entry rules (`CatalogEntry.fromJson`):
- must be an object with string `id` and string `version`, else dropped;
- must have (`url` AND `sha256`) OR `git`, else dropped (a zip without hash is rejected);
- `name` defaults to `id`; `description`, `author` default `""`; `maestria` (int, API version) defaults to the host API (`mxPluginApi` = 1); `permissions` = string items only; `kinds` = known ids only (`theme`, `feature`; unknown strings ignored; missing → empty set, no tag shown); `icon` optional (URL of an `.svg`);
- when `git` is present, `url`/`sha256` are ignored (set null); `ref` optional; `sha256` lower-cased.
- `fits` = `maestria <= hostApi`.

**`PluginKind`** (new enum, also used by the installed list): `theme` label `Tema`, `feature` label `Recurso`. For an installed manifest, `kinds` = {theme if `themes` non-empty} ∪ {feature if has process, or commands, settings, `sidebar`, `screen`, or `suggestions` declared}. Empty manifest → empty set.

**`PluginOrigin`** (where future versions come from): either `{catalog: <catalog.json URL>}` or `{git: <repo URL>, ref?: <branch>}`. `.zip` and local folders have no origin ("uma foto"). Equality by all three fields.

### B.2 — Update checking (`PluginUpdates`)

Timings: first check `45 s` after startup (`firstDelay`), then every `6 h` (`every`, `Timer.periodic`). Not started under tests. A check is skipped if one is already running.

`check()`:
1. `checking=true`, `error=null`, notify.
2. For each catalog URL: `load(url)`; on failure `error = 'Não deu pra ler o catálogo: <e>'` (keeps going).
3. For each installed plugin with a manifest and **not linked** (dev links are never touched):
   - `_lookup` → candidate update (errors are logged in the plugin log as `procurar versão nova: <e>`, not fatal);
   - `_offer` → adoption offer.
   Replace `available` and `offers` maps wholesale; `checkedAt = now`; `checking=false`, notify.
4. For each available update where `entersAlone(u)`: install quietly (sequentially).

`_lookup(p, m)`:
- origin catalog: entry `byId` in the loaded catalog; none, `!fits`, or `compareVersions(entry.version, m.version) <= 0` → none. Else update `{from: m.version, to: entry.version, origin, entry, asksMore: any entry permission not in manifest permissions}`.
- origin git: `head = git ls-remote <git> <ref or HEAD>` run with `GIT_TERMINAL_PROMPT=0` (private repo without credentials fails instead of prompting); non-zero exit or empty → none; head = first whitespace-separated token of stdout. If `_gitSeen[id]` has the same head → return the cached conclusion (no clone). Else `stage(git, ref)` (shallow clone), compare: update iff staged id == plugin id and staged version > installed; `asksMore = asksMore(old, next)`; cache `(head, update)`; always discard the staging dir.
- no origin → none.

`_offer(p, m)`: only when origin is not a catalog; first catalog containing the id with `fits` and `compareVersions(entry, installed) >= 0` (same version is offered too) → `PluginUpdate{origin: catalog(url), entry}`.

`asksMore(old, next)` (static) = `next.permissions − old.permissions` non-empty OR (`!old.hasProcess && next.hasProcess`).

`entersAlone(u)` = `plugins.autoUpdate && id ∉ manualUpdate && !u.asksMore && u.newer`.

**Quiet install** (`_quietly`): skip if plugin gone / no manifest / already busy. Mark busy; `stageUpdate(u)`; **re-check `asksMore(old, staged.manifest)` against the real downloaded manifest** (catalog may under-declare) — if true, discard and store `available[id] = u` with `asksMore=true`; else `finish(staged)` and log `atualizado sozinho de <from> pra <to>`. Errors → plugin log `atualizar pra <to>: <e>`. Unmark busy.

**`compareVersions(a,b)`** (shared with app updater, `updater.dart`): strip leading `v`, split on `[.+-]`, each part `int.tryParse` else 0, compare numerically padding with 0; returns -1/0/1. (So `1.2.0-beta` == `1.2.0.0`; pre-release suffixes are not ordered semver-style — replicate exactly.)

**Staging a catalog entry** (`stageEntry(e, catalogUrl)`), origin = `catalog(catalogUrl)`:
- git entry: `plugins.stage(git, ref, origin)`, then id check.
- zip entry: temp dir `root/.staging/dl*`; download `e.url` to `<tmp>/<id>.zip` (non-200 → `PluginInstallError('<url>: HTTP <code>')`); sha256 via `shasum -a 256` (macOS) / `sha256sum` (Linux), first token lower-cased (tool failure → `sha256: <stderr>`); mismatch → `O pacote de <name> não confere (sha256 <got>)`; then `plugins.stage(zipFile, origin)` and rebuild the StagedPlugin with `source = e.url` (so the install log reads `instalado de <url>`, not the temp path); always delete the dl temp dir.
- id check (`_sameId`): staged manifest id != entry id → discard, `O pacote é de "<got>", e não de "<id>"`.

`stageUpdate(u)`: catalog entry → `stageEntry`; else `stage(origin.git, ref)`.
`prepare(u)` (UI path): if id already busy → `PluginInstallError('Já está baixando')`; busy during download.
`finish(staged)`: `store.installPlugin(staged)` (commit + close old views + save), then remove id from `available`, `offers`, `_gitSeen`; notify. **All installs (URL dialog too) now go through `finish`.**

### B.3 — Changes to install/commit/uninstall and persistence

- `stage(source, {origin, ref})`: git clone becomes `git clone --depth 1 [--branch '<ref>'] '<src>' '<into>'`. Resulting `StagedPlugin.origin` = explicit origin, else `git(src, ref)` if the source is git, else null.
- `commit`: after moving the dir, `origins[id] = staged.origin` or **removed** when null (replacing a catalog plugin by a zip/folder stops catalog updates).
- `link` (dev): `origins.remove(id)`.
- `uninstall`: also `origins.remove(id)` and `manualUpdate.remove(id)`.

**config.json** — inside the existing `plugins` object (§1.6 of 04):
| key | type | default | written when |
|---|---|---|---|
| `plugins.autoUpdate` | bool | `true` | only `false` is written; read as `!(json.autoUpdate == false)` |
| `plugins.manualUpdate` | string[] | `[]` | non-empty, sorted |
| `plugins.origins` | `{ <id>: {catalog} \| {git, ref?} }` | `{}` | non-empty, keys sorted; entries that are not objects or lack both keys are ignored |

Back-compat: a 2.4 config has none of these → auto-update on, no origins (so plugins installed under 2.4 from git get no updates until reinstalled; they can still be *offered* adoption by the catalog). A 2.4 reader ignores the keys. **Do not confuse** with the new top-level `autoUpdate` (app updater, default `false`, written only when `true`) — section A.

Store: `setPluginAutoUpdate(on)` — no-op if unchanged; set, save, notify; turning **on** triggers an immediate `check()`. `setPluginManualUpdate(plugin, manual)` — add/remove, save, notify.

### B.4 — UI

**Entry points**: Settings → Plugins button row gains first button `Catálogo` (icon storefront) before `Instalar…`. The rail's faint `+` at the end (tooltip still `Instalar plugin`) now opens the **catalog**, not the URL install dialog.

**Catalog dialog** (`showPluginCatalog`): title `Catálogo de plugins` + 14px spinner while loading; content 560×460. On open always reloads the official catalog from the network (shows the in-memory copy meanwhile). No catalog yet and error → centered red `Não deu pra ler o catálogo: <e>` + button `Tentar de novo`; no catalog and no error → empty. Actions: `De uma URL, .zip ou pasta…` (closes and opens the old install dialog), `Fechar`. Rebuilds on store changes.

Entry card (rounded 9, border, bg): row = icon (catalog `.svg` fetched from network, 17px, tinted `fgDim`; non-svg/missing/error/loading → generic extension icon) + wrap [name (12.5 w600) · version text (`<have> → <new>` when installed and different, else `<new>`; mono 11 faint) · kind tags · `De fora` (faint) when the entry is a git entry]; below (indent 28): description (max 2 lines), `Pode <label1>; <label2>` in yellow (permission labels, unknown ids shown raw; max 2 lines), per-entry error in red. Trailing action, first match:
1. busy (this dialog or a background update) → 16px spinner;
2. installed and linked → `Desenvolvimento` (purple);
3. `!fits` → `Pede uma MaestrIA\nmais nova` (right-aligned, dim);
4. installed and catalog version <= installed → `Instalado`;
5. else tonal button `Instalar` (not installed) / `Atualizar`.
Install click: `stageEntry` → `confirmStagedPlugin` (trust dialog); errors shown under the entry. If the dialog closed meanwhile, the staging is discarded.

**`PluginKindTag`**: pill (radius 4, padding 6×1.5, bg = color @12% alpha), icon 11px + label 10.5px; theme → accent colour, palette icon, `Tema`; feature → `fgDim`, extension icon, `Recurso`. Shown in catalog entries and next to installed plugin name in Settings (from manifest `kinds`, order theme, feature).

**`confirmStagedPlugin`** (trust dialog reused): title `Instalar <name>?` / `Atualizar <name>?`, width 480, body `PluginTrust`; buttons `Cancelar` (discard staging) / `Confio, instalar` / `Confio, atualizar`. On confirm `finish` and banner `<name> <version> instalado` / `<name> atualizado de <old> pra <new>`.

**`updatePlugin(u)`** (menu "update"/"adopt"): `prepare(u)`; failure → sticky banner `Não deu pra baixar a <to>: <e>`. If origin unchanged (`origins[id] == u.origin`) and the downloaded manifest does not ask more → install directly + banner; otherwise (origin switch = adoption, or asks more) → trust dialog.

**Settings → Plugins, auto-update block** (`_AutoUpdate`, below the buttons): toggle `Atualizar os plugins sozinho` (2.6.0 had subtitle `Os que vieram do catálogo ou de um repositório de git ganham as versões novas sem perguntar. A que pede uma permissão nova espera o seu "confio". Dá pra deixar um plugin de fora no menu dele.`; **2.7 dropped the subtitle** when the row became `_Toggle`). Below: status text `Procurando…` while checking, else `Procurado às HH:MM` (local, zero-padded) once checked, else nothing; button `Procurar agora` (disabled while checking); red error line when `error != null`.

**Plugin row** (Settings): after state label / `Desenvolvimento`: `Atualizando…` (purple) when busy, else if update available `<to> pede permissão nova` (yellow, asksMore) / `<to> disponível` (purple). Details list gains `Vem do catálogo oficial` / `Vem do catálogo <url>` / `Vem de <git>` / `Vem de <git> (<ref>)` when an origin exists.

**Plugin row menu** — new items inserted before `Log`:
- `Atualizar pra <to>` (icon system_update, purple; subtitle yellow `Pede mais do que a <from> pedia` when asksMore) — when an update is available and not busy → `updatePlugin`.
- `Receber pelo catálogo` (storefront icon; subtitle `Instala a <to> de lá` if newer, else `A mesma <to>, e as próximas de lá`) — when an offer exists and not busy → `updatePlugin(offer)`.
- `Atualizar sozinho` checkbox item (checked = not in manualUpdate; subtitle `Desligado pra todos, acima` when global toggle off) — only when the plugin has an origin → toggles `manualUpdate`.

Shortcuts: none.

### B.5 — Tests (given/when/then)

Fixture: a local HTTP server serving `catalog.json` (`{schema:1, plugins:[…]}`) and zips (`<id>-<version>.zip` containing the plugin folder, as the maestria-plugins workflow packs them); 404 otherwise. Catalog list injected = `[fake URL]`; install fn = raw `commit`.

1. Parse: given entries a(zip+`sha256:"AB"`, kinds `[theme, nova]`), b(zip, no sha), c(git+ref, no kinds), d(git, maestria 99) → ids `[a,c,d]`; a.sha256 `ab`; a.kinds `{theme}`; c.kinds empty; c.git set and url null; d.fits false. Parsing `[]` throws a format error.
2. Install from catalog: publish `mx.git` and `mx.ssh` 1.0.0; install `mx.git` via load+stageEntry+commit → installed version 1.0.0, only `mx.git` on disk, `origins[mx.git] = catalog(url)`, last log line contains `mx.git-1.0.0.zip`, `.staging` empty.
3. Bad hash: entry sha256 = 64×`0` → install throws install error, nothing installed, `.staging` empty.
4. Auto: installed 1.0.0 from catalog, publish 1.1.0, `check()` → installed 1.1.0, `available` empty, origin unchanged.
5. Manual: plugin in `manualUpdate` → check leaves 1.0.0 and `available[mx.git].to == 1.1.0`; clear manual, `autoUpdate=false` → still 1.0.0; `finish(prepare(u))` → 1.1.0, `available` empty.
6. New permission: publish 2.0.0 with `permissions:[hooks]` → check leaves 1.0.0, `available.asksMore == true`.
7. Lying catalog: publish 2.0.0 with `terminal.write` but catalog entry permissions `[]` → check leaves 1.0.0, `asksMore == true`, `.staging` empty (verified against downloaded manifest).
8. Git origin: local git repo in a dir named `lucas.git` (branch main) installed via `stage(path)` → `origins = git(path)` (it counts as git only through the `.git`-suffix rule — the Rust fixture must use the same name); check with no new commit → `available` empty; bump version + commit → check installs 0.2.0.
9. Offer: git-installed `com.lucas.foo` 0.1.0, catalog publishes same 0.1.0 → `offers[id].newer == false`; `finish(prepare(offer))` → origin becomes catalog(url), `offers` empty.
10. Dev link: linked `mx.git` 0.0.1, catalog has 9.0.0 → `available` and `offers` empty.
11. Config: after install + `autoUpdate=false` + manual `{mx.git}`, JSON round-trip into a fresh registry → autoUpdate false, manual `{mx.git}`, origin catalog(url); `load({})` → autoUpdate true. Reinstalling `mx.git` from a folder → `origins` empty. Uninstall → `manualUpdate` empty.
12. Manifest kinds: themes only → `{theme}`; `main` only → `{feature}`; themes+commands → both; empty → empty.
13. Rail: no plugins, tap `+` → text `Catálogo de plugins` visible (do not wait for network); tap `De uma URL, .zip ou pasta…` → `Instalar plugin` visible.

### B.6 — Rust landing

mx-plugins (pure logic, no gpui):
- NEW `crates/mx-plugins/src/catalog/mod.rs` — `CatalogEntry`, `PluginCatalog::parse`, `OFFICIAL_CATALOG`, `PluginKind`.
- NEW `crates/mx-plugins/src/catalog/updates.rs` — `PluginUpdates` (timers, `check`, `lookup`, `offer`, `asks_more`, `enters_alone`, `stage_entry`, `prepare`, `finish`, quiet install, busy/available/offers/git_seen/checked_at/error state, change listener).
- NEW `crates/mx-plugins/src/catalog/fetch.rs` — HTTP GET/download (reqwest with redirects, `User-Agent: maestria`), sha256 (prefer `sha2` crate over shelling out — behaviour-equivalent; lower-case hex), `git ls-remote` with `GIT_TERMINAL_PROMPT=0`.
- NEW `crates/mx-plugins/src/catalog/tests.rs` — cases 1–11 (axum or tiny hyper server on 127.0.0.1:0; `zip` via the `zip` CLI or crate).
- EDIT `crates/mx-plugins/src/registry/install.rs` — `stage(source, origin, ref)`, `--branch`, `StagedPlugin.origin`, origins update in `commit`/`link`/`uninstall`, `manual_update` removal.
- EDIT `crates/mx-plugins/src/registry/persist.rs` and `registry/mod.rs` (Reg fields `origins: BTreeMap<String, PluginOrigin>`, `auto_update: bool = true`, `manual_update: BTreeSet<String>`).
- EDIT `crates/mx-plugins/src/manifest/mod.rs` — `kinds()`.
- Shared `compare_versions`: put in `crates/mx-core/src/version.rs` (NEW) so the app updater (section A) and the catalog share it; coordinate — whichever package lands first owns the file.
- Cargo: `crates/mx-plugins/Cargo.toml` add `reqwest`, `sha2` (workspace).

mx-store:
- EDIT `crates/mx-store/src/features.rs` or NEW `crates/mx-store/src/plugin_updates.rs` — own the `PluginUpdates` instance, start after 45 s (not in tests), `set_plugin_auto_update` (save + check on enable), `set_plugin_manual_update`, `install_plugin` path via `finish` (closes old views).

mx-app:
- NEW `crates/mx-app/src/plugin_ui/catalog.rs` — catalog dialog, entry cards, `kind_tag`, svg icon fetch with fallback.
- EDIT `crates/mx-app/src/plugin_ui/install.rs` — split out reusable `confirm_staged` trust dialog; commit through `finish`.
- EDIT `crates/mx-app/src/plugin_ui/mod.rs` — `open_catalog`, `update_plugin`.
- EDIT `crates/mx-app/src/rail/mod.rs` — `+` opens catalog.
- EDIT `crates/mx-app/src/chrome/settings/plugins.rs` — `Catálogo` button, auto-update block, row status/origin/kind tag, three new menu items (+ `PluginMenuAction` variants).

### B.7 — Already in Rust / discrepancies

- `registry/install.rs`: stage/commit/discard/link/uninstall exist and match 2.4. Missing: `origin`/`ref` on stage and `StagedPlugin`, `--branch`, origin bookkeeping, `manual_update` cleanup on uninstall.
- `run_tool` uses `env_clear().envs(plugin env)`: `git ls-remote` must add `GIT_TERMINAL_PROMPT=0` explicitly.
- `registry/persist.rs`: only `disabled` and `settings`; add the three keys with exact omission rules.
- `rail/mod.rs:120-123`: `+` calls `open_install` — must call `open_catalog` (tooltip unchanged).
- `chrome/settings/plugins.rs::plugin_menu(conflicts)` has no update/adopt/auto items; signature needs update/offer/manual/origin/auto state.
- No `compare_versions`, no HTTP client in mx-plugins, no catalog, no kinds, no sha256 anywhere.


## C — Terminal-side changes (reflow copy, sticky selection, find in pane)

Hunk attribution for the terminal files (2.4 → 2.7):

| File | Commit | Content | Owner in this delta |
|---|---|---|---|
| `lib/services/vt.dart` | fcc695b (BUG#80709) | `selectedText(..., reflow:)` + helpers | C1 |
| `lib/services/vt.dart` | d77be94 | 7th emulator fix: reattach detached screen lines after each `write` | C2 |
| `lib/services/pty.dart` | e570922 (BUG#80709) | `copySelection({reflow})` | C1 |
| `lib/services/pty.dart` | d77be94 | constructs `StickySelection(terminal, controller)`; temporary DEBUG log in `wheel()` (removed by 610086e) | C2 |
| `lib/services/pty.dart` | c707872 (TASK#82252) | `pid => _pty?.pid ?? fakePid` (`fakePid` test-only) | area E (history attach) — test seam only |
| `lib/services/pty.dart` | b043ae0 (TASK#82254) | `asksTrust`, `screenText()`, `enter()`, `sendKeys(keys, gap=60ms)`, `terminalKeys` map | area F (sessions API) — listed here for cross-ref only, see §C4 |
| `lib/services/pty.dart` | 610086e | removes the DEBUG log lines | none |
| `lib/ui/terminal_pane.dart` | e570922 | ⌘C passes `reflow: tab.kind == claude` | C1 |
| `lib/ui/terminal_pane.dart` | 28eefb2 (TASK#82251) | find bar overlay, own `ScrollController`, `TerminalFind` registry, ⌘F handled in surface | C3 |
| `lib/ui/terminal_pane.dart` | 5c7af80 (TASK#82250) | `PaneGrab` around header, per-panel hotkey in surface key handler, `PaneKeyHint(chord: tab.hotkey)`, `EmptyPane` rewritten to session cards | area E |
| `lib/ui/terminal_pane.dart` | 610086e (TASK#82249) | header loses the dirty-count chip and the subtitle text | area E |
| `lib/ui/dialogs.dart` | e570922 | terminal context menu "Copiar" passes `reflow: tab.kind == claude` | C1 |
| `lib/services/shortcuts.dart`, `lib/ui/keys.dart` | 28eefb2 | new `findInPane` action; `search` rebound | C3 |
| `test/sidebar_toggle_test.dart` | 28eefb2 | sidebar search test now presses ⌘⇧F | C3 |

Note for porting: `lib/services/sticky_selection.dart` at origin/main still contains a `_log()` "DEBUG temporário -- remover" writing to a hard-coded macOS `/private/tmp/claude-501/...sticky.log` from `_selected` and `_follow`. Do NOT port it.

---

### C1 BUG#80709 — Copy with reflow in Claude tabs (fcc695b, e570922)

#### Behaviour

`selectedText(terminal, range, reflow: bool = false)`. Without reflow: unchanged 2.4 behaviour (rows joined by `\n`, except that a row with `isWrapped` — soft-wrapped by the emulator — is appended to the previous line with no separator). With `reflow = true` the rows Claude Code wrapped itself (hard newlines, so `isWrapped` is false) are joined back, and Claude's layout indent/reply bullet is dropped.

Where reflow is used: every copy from a tab whose `kind == claude` — the ⌘C / Ctrl+Shift+C key in the terminal surface and the terminal context menu item `Copiar`. Shell/launcher tabs copy without reflow. (No config key, no new shortcut.)

Exact algorithm (port 1:1; "blank" = code point 0 or 0x20; widths per cell, wide char = 2):

```
selection = range.normalized
lines: list of list<string>  = [[]]
hanging = 0
for segment in selection.toSegments():            # one per buffer row, [start,end) cells
  skip if segment.line out of [0, buffer.height)
  line  = buffer.lines[segment.line]
  text  = segmentText(line, start, end)           # 2.4 helper: null cells → spaces, trailing unwritten dropped
  first = segment.line == selection.begin.y || segment.line == 0
  if first or line.isWrapped:
     lines.last.add(text)
     if first: hanging = hangingIndent(line)
     continue
  above = buffer.lines[segment.line - 1]
  if reflow and claudeWrapped(above, line, viewWidth, hanging):
     space = brokenWord(above, viewWidth, hanging) ? "" : " "
     lines.last.last = lines.last.last.trimRight()
     lines.last.add(space + text.trimLeft())
     continue
  lines.add([text]); hanging = hangingIndent(line)
text = [join(parts) for parts in lines]
if !reflow: return text.join("\n")
fromIndent = begin.y in range && begin.x <= indent(buffer.lines[begin.y])
return dedent([l.trimRight() for l in text], firstCounts: fromIndent).join("\n")
```

Helpers:
- `indent(line)`: count of leading blank cells.
- `drawnEnd(line)`: column right after the last non-blank cell (`i + width(i)`), 0 if none.
- `wordWidth(line, from)`: advance from `from` while non-blank, step `clamp(width(i),1,2)`; returns cells consumed.
- `hangingIndent(line)`: `i = indent`; scan the first word `[indent, i)`; if that word matches `^([●⏺•⎿❯>*-]|\d+[.)])$` then skip the following blanks and return that column, else return `indent`. (So list items `- `, `1. `, `2) ` and reply bullets `● ` give a hanging indent at the text start.)
- `claudeWrapped(above, below, width, hanging)`: `end = drawnEnd(above)`, `ind = indent(below)`; false if `end == 0 || ind >= below.length || ind != hanging`; false if `above[end-1]` or `below[ind]` is box-drawing U+2500..U+257F (tables are never prose); else true iff `end + 1 + wordWidth(below, ind) > width` (the next row's first word would not have fit after a space).
- `brokenWord(line, width, hanging)`: `drawnEnd(line) >= width && hanging + wordWidth(line, hanging) >= width` (a single word longer than the row, cut mid-word → join without space).
- `dedent(lines, firstCounts)`: `kept = {0}` if `!firstCounts`; for every line matching `^[●⏺] +` strip that match and add the index to `kept`. `shared = min(leading-space count)` over non-kept, non-blank lines; if none, return as is. Every non-kept line loses up to `shared` leading 0x20 chars (`cutIndent`). Kept lines are untouched.

Clipboard write: `copySelection({reflow})` → if a selection exists, clipboard text = `selectedText(..., reflow)`.

#### Tests (copy_test.dart, group "selectedText with reflow")

Fixture: `claude(rows, width=40)`: terminal 40×12, writes `ESC[H● ` + rows[0]; each next row as `\r\n` + (`ESC[2C` + row, or nothing if row empty) — i.e. continuation at column 2 via cursor-forward (how Claude Code 2.1.292 writes). `reflowed(from=0, line=0, lastLine)` = selection from `(from,line)` to `(viewWidth,lastLine)` with reflow.

1. Given rows `Rivers shape landscapes and sustain` / `ecosystems across the whole globe.`, when reflowed 0..1, then `Rivers shape landscapes and sustain ecosystems across the whole globe.` (joined, no indent, no `●`).
2. Same rows, without reflow (range (0,0)-(40,1)) → `● Rivers shape landscapes and sustain\n  ecosystems across the whole globe.`
3. Rows `First paragraph.`, ``, `Second one.` → `First paragraph.\n\nSecond one.` (a short row ends a line).
4. Rows `Short line`, `x` → `Short line\nx` (next word would have fit → separate line).
5. Rows `Two items:`, ``, `- The first item is long enough to`, `  wrap.`, `- The second.` → `Two items:\n\n- The first item is long enough to wrap.\n- The second.`
6. Rows `Rivers shape landscapes and sustain`, `ecosystems.`, ``, `- An item.`, selection from column 9 → `shape landscapes and sustain ecosystems.\n\n- An item.` (start on a word: first line not counted for indent, rest dedented).
7. Rows `https://example.com/a/very/long/path/x`, `yz` → `https://example.com/a/very/long/path/xyz` (broken word, no space).
8. Rows `The release shipped in the spring of`, `2026. After that the whole team had`, `moved on.` → `The release shipped in the spring of 2026. After that the whole team had moved on.` (`2026.` matches the marker regex, but `hanging` is only recomputed on rows that start a new line; a joined row keeps the hanging indent of the line it joined, so the number does not break the sentence).
9. Rows `First reply.`, ``, `ESC[1G● Second reply.`, `  - item` → `First reply.\n\nSecond reply.\n- item` (both `●` dropped).
10. Selection from column 8 (the space between words) of the case-6 rows → ` shape landscapes and sustain ecosystems.\n\n- An item.` (leading space kept; starting inside the text is not "from indent").
11. Box table rows `┌──…┐`, `│ a long table row that fills it  │`, `└──…┘` → result has exactly 3 lines.
12. (pre-existing group "⌘C") copies the selection, spaces and all — unchanged contract, now through `copySelection(reflow: true)` for a claude tab.

#### Rust landing

- `crates/mx-term/src/reflow.rs` (new): pure fn `selected_text(grid rows view, range, width, reflow: bool) -> String` over a minimal row trait (`len`, `code_point(i)`, `width(i)`, `is_wrapped`) so it is unit-testable without alacritty; port the 11 cases as Rust tests with a tiny VT fixture (feed bytes to an alacritty `Term`).
- `crates/mx-term/src/session.rs`: `selected_text(&self, reflow: bool)` — build segments from `term.selection.to_range()`; alacritty's `WRAPLINE` flag on the last cell of the previous row = Dart `isWrapped` of the next row.
- `crates/mx-app/src/terminal_view.rs` (`copy_selection`) and `crates/mx-app/src/terminal_glue.rs` (`TermMenu::Copy`): pass `reflow = self.claude`.

#### Already in Rust / discrepancies

- `Session::selected_text` uses alacritty `selection_to_string()` — no reflow, and alacritty's own wrap/blank handling differs from the Dart `segmentText` (spec 02 §5 item 5 already flags blank-cell handling). Reflow must be built on a cell-level reader, not on `selection_to_string`.
- `TerminalView::copy_selection` and the context-menu copy take no tab kind; `TerminalView` already has `self.claude` (used for paste) — reuse it.

---

### C2 Sticky selection on scroll (alternate screen) + 7th emulator fix (d77be94)

#### Behaviour

Problem: in the alternate screen (where `claude` lives) "scrolling" is the program repainting the same rows with other text; a selection anchored to row objects stays fixed on screen while text moves under it. Fix: remember the text of the selected rows and, after the screen changes, find where it went; move the selection there, or clear it if gone. Only while `isUsingAltBuffer`; on the main screen nothing changes (scrollback anchors already follow).

State: `begin`, `end` (raw drag ends, `begin` may be after `end`), `rows` = full text of every row from `top=min(y)` to `bottom=max(y)` (whole rows, not just the selected span), `check` timer, `missed` flag, `moving` (re-entrancy guard while we set the selection ourselves).

Row text normalisation `text(line)`: every cell, code point 0 → space, then `trimRight()` (a cursor-skipped blank and a written space compare equal).

On selection change (not by us): if no selection or not in alt screen → forget. If begin/end unchanged → ignore. Else store begin/end, `rows = read(top..bottom)`, `missed=false`.

On terminal write: if `rows != null` and no timer pending → schedule `follow` in **30 ms** (`_settle`; a bound, not a debounce — while Claude streams, it still follows every 30 ms).

`follow()`:
1. If not in alt screen → forget.
2. `current` = live selection (may already have moved with a real scroll); `begin/end = current ?? saved`.
3. `shift = find(rows, top(begin,end))`: for `distance = 0..height-1`, try shifts `[0]` then `[+d, -d]`; skip if `at<0 || at+rows.len > height`; first exact match (all rows equal by `text`) wins — smallest displacement first, positive before negative.
4. `shift == null`: if `!missed` → `missed=true`, retry after **120 ms** (`_grace`, a frame split across pty reads); else clear selection and forget.
5. `missed=false`. If `shift == 0 && current != null` → just re-save begin/end.
6. Else set selection to `(begin.x, begin.y+shift)`–`(end.x, end.y+shift)` (guarded by `moving`), save them.

7th emulator fix (vt.dart top-of-file list item "A scroll, and the lines that no longer know they are on screen"): xterm's `scrollUp/scrollDown/deleteLines` detach line objects so anchors on them die (selection vanishes, OSC 8 links not found) after `CSI S`/`CSI T`, LF under DECSTBM, or LF at the bottom of the alt screen. `VtTerminal.write` override: after `super.write`, for each of the last `viewHeight` lines, if `!attached` re-assign it. Rust: alacritty rotates `Selection` on in-region scroll (`Selection::rotate`) and has no detached-anchor concept — the fix is N/A, but the observable contract (test 4 below) must hold.

#### Tests (selection_test.dart, group "rolar com texto selecionado na tela alternativa")

Fixture `frame(first, rows=20)`: for y in 0..rows: `ESC[{y+1};1H` + `msg {first+y} do transcript` + `ESC[K`. `select(row, to)` selects whole rows. Terminal entered with `ESC[?1049h`. Pumps of 50 ms or 300 ms.

1. Given alt screen with frame(0), row 5 selected (`msg 5 do transcript`); when frame(3) is written and 50 ms pass, then selected text still `msg 5 do transcript` and `begin.y == 2`; when frame(-4) is written and 50 ms pass, then text unchanged and `begin.y == 9`.
2. Given row 5 rewritten as `uma` `frase` `com` `buracos` with cursor jumps (`ESC[6;1H ESC[2K uma ESC[6;5H frase ESC[6;11H com ESC[6;15H buracos ESC[K`) and selected; when frame(3) then `ESC[3;1Huma frase com buracos ESC[K` (spaces really written), 50 ms → selection present, `begin.y == 2`, text `uma frase com buracos`.
3. Given rows 4..6 selected; frame(2), 50 ms → same text, `begin.y == 2`, `end.y == 4`.
4. Given frame(0, rows 24), row 10 selected; `ESC[3S` (CSI scroll up 3), 50 ms → text `msg 10 do transcript`, `begin.y == 7`.
5. Given row 12 selected; write `ESC[2J` + first half of frame(3) (up to `ESC[6;1H`), 50 ms, then the rest, 300 ms → text `msg 12 do transcript`, `begin.y == 9` (one miss is tolerated).
6. Given row 1 selected; frame(10), 300 ms → selection is null (text scrolled off).
7. Main screen: 10 lines `linha i\r\n`, row 3 selected; `ESC[4;1Houtra coisa ESC[K`, 300 ms → selection still present, `begin.y == 3`.
Pre-existing group "arrastar o mouse" (2 cases) unchanged.

#### Rust landing

- `crates/mx-term/src/sticky.rs` (new): `StickySelection` state + pure `find_shift(rows: &[String], top, screen_rows: impl Fn(usize)->String, height) -> Option<i32>` and `row_text`.
- `crates/mx-term/src/session.rs`: hook "selection changed" (start/update/clear_selection paths) and the output path (`io.rs` reader → after each chunk processed) to schedule follow at 30 ms / 120 ms via the session's existing timer/event mechanism (`events.rs`), setting the selection with the `moving` guard. Only when `TermMode::ALT_SCREEN`.
- Coordinates: alacritty `Point.line` is viewport-relative (`Line(0)` = top of screen, negative = history); alt screen has no history so screen row = line.

#### Already in Rust / discrepancies

None implemented. alacritty clears the selection on many writes that touch selected cells (`Term::selection` is cleared by `clear_screen`, `ED`, and when writing into the selected region in some versions) — verify: Dart test 5 relies on the selection surviving `ESC[2J`; in Rust the sticky module must re-create it from saved begin/end when alacritty dropped it (treat `current == None` as "use saved", exactly like Dart).

---

### C3 TASK#82251 — Find in the pane terminal (⌘F) (28eefb2)

#### Behaviour

Search engine `findInTerminal(terminal, query, limit=1000) -> [TermHit{from, to}]` (to exclusive, may be on a later row):
- Empty query → `[]`.
- Literal match (regex-escaped; punctuation is text, not regex). Smart case: case-sensitive iff `query != query.toLowerCase()`.
- Searches by *logical* line: a row plus all following rows with `isWrapped` are concatenated before matching (a word split by soft wrap is found; one hit spanning two rows is one hit).
- Text built cell by cell over `[0, min(viewWidth, line.length))`: skip the zero-width trailing half of a wide char (`code==0 && x>0 && width(x-1)==2`); code 0 → space; a non-BMP char contributes two string units mapped to the same cell. A parallel `cells[]` maps string index → `(y,x)`.
- For each match: `from = cells[m.start]`, `end = cells[m.end-1]`; `to = (end.x + (width(end)==2 ? 2 : 1), end.y)`. Stop at `limit` hits. Order: top to bottom (whole buffer, scrollback included).

`FindSession` (per open bar): `query`, `hits`, `index` (-1 when none), highlights = one per hit, anchored to lines (so they follow scrollback growth/trim).
- `search(query, nearLine)`: recompute; `index = last hit with from.y <= nearLine`; if none and hits exist → 0. `nearLine` = last buffer line visible on screen (`floor((scroll.pixels + viewport)/lineHeight)`, clamped; = last line when no scroll info). I.e. it starts at the most recent hit at/above the bottom of the viewport.
- `refresh()` (new output while open): if query empty, nothing. Read the current hit's anchor offset (if still attached); recompute; if no hits → -1; else if no previous anchor → last hit; else first hit not before the previous offset (or last if none). The current hit does not jump to new output.
- `step(by)`: `index = (index + by) mod len` (wraps both ends); no-op without hits.
- `clear()`, dispose removes highlights.
- Highlight colour: `Mx.yellow` at alpha **0.62** for the current hit, **0.24** for the others (painted over the text).

Panel integration (terminal surface):
- Opening (⌘F with focus in that terminal, or global ⌘F routed to the focused tab): create the session if needed, start listening to output, select all text in the field, focus the field next frame. A second ⌘F re-focuses the field with its text selected.
- Typing: debounce **100 ms** then `search(text, nearLine: bottomLine)` then reveal.
- Output while open: debounce **250 ms** then `refresh()`.
- Enter (in field) = previous hit (`step(-1)`); Shift+Enter = next (`step(+1)`); if the 100 ms debounce is still pending, Enter/Shift+Enter first runs the pending search instead of stepping. Esc closes.
- Reveal: if the current hit (rows from.y..to.y) is not fully inside the viewport, jump-scroll so its top row is vertically centred: `top - (viewport - lineHeight)/2`, clamped to `[0, maxScroll]`. No animation.
- Close: stop listening, cancel refresh timer, dispose session (highlights gone), refocus the terminal. The field text is kept (next ⌘F shows the previous query selected).
- Surface reused for another tab (sidebar swaps what the grid slot shows): unregister/register under the new tab id; an open find is dropped.
- A `TerminalFind` registry maps `tabId → open callback`; unregister only if the callback still matches (a moved pane registers before the old one leaves).

Find bar layout: overlay `Positioned(top: 8, left: 12, right: 12)` aligned top-right inside the terminal area, outside the pty pointer listener (clicks on the bar never reach the terminal). Container 300×34, padding left 10 / right 4, bg `bgSidebar`, radius 8, 1px border `border` (or `red` when query non-empty and 0 hits), shadow `shadow` blur 10 offset (0,3). Row: search icon 14 `fgFaint`; 6px; text field 12px `fg`, cursor `accent` width 1.5, no border, hint `Buscar no painel` (12px `fgFaint`); when query non-empty a counter 11px tabular figures: `Nenhum` (red) or `{index+1} de {count}` with suffix `+` when count ≥ 1000 (`fgFaint`), 6px horizontal padding; three 24×24 buttons radius 6, icon 16, hover bg `border`, icon `fgDim` (hover `fg`), disabled `fgFaint@0.4`, tooltip wait 400 ms:
  - up arrow `keyboard_arrow_up_rounded`, tooltip `Anterior  ⏎` (two spaces), disabled when 0 hits;
  - down arrow, tooltip `Próximo  ⇧⏎` (Linux: `Próximo  Shift+Enter`), disabled when 0 hits;
  - close, tooltip `Fechar  ⎋` (Linux: `Fechar  Esc`), always enabled.
The pty stays in a `Stack` even when the bar is closed (stable tree position).

#### Shortcuts / keymap (shortcuts.dart, keys.dart)

- New action `findInPane`: id `find`, label `Buscar no painel`, hint `No que o terminal em foco já escreveu; Enter sobe, Shift+Enter desce, Esc fecha`, group navigation, default `⌘F`, Linux `Ctrl+Shift+F`. Placed before `search` in the enum.
- `search` (Buscar na lateral) rebound: default `⇧⌘F` (was ⌘F), Linux `Ctrl+Alt+F` (was Ctrl+Shift+F).
- Inside a terminal surface, `findInPane` is handled locally (opens this surface's find, also in plugin-window terminals). Global dispatch (focus elsewhere): `TerminalFind.open(store.focusedTab)`; if no focused tab or no registered surface → banner `Nenhum terminal em foco pra buscar`.
- Users with a persisted keymap override for `search` keep it (keymap persistence stores only overrides — see spec 01); a conflict check is not added.

#### Tests (terminal_find_test.dart)

Engine:
1. `erro aqui\r\nnada\r\noutro erro` (40 cols), `erro` → hits `(0,0)-(4,0)` and `(6,2)-(10,2)`.
2. `Erro ERRO erro`: `erro` → 3 hits; `Erro` → only `(0,0)-(4,0)`.
3. `0123456789abc` at 10 cols (row 1 isWrapped): `89ab` → `(8,0)-(2,1)`.
4. `a你好b`: `好b` → `(3,0)-(6,0)`; `你好` → `(1,0)-(5,0)`.
5. `a.b axb`: `a.b` → 1 hit (literal).
6. `x x x x x`: limit 3 → 3 hits; empty query → none.
Shortcuts:
7. macOS: `findInPane` defaults label `['⌘F']`; `search` `['⇧⌘F']`.
8. Linux: `findInPane` `['Ctrl+Shift+F']`; `search` `['Ctrl+Alt+F']`; all chords valid.
Bar (pane grid with global keymap; terminal has `primeiro erro\r\nnada aqui\r\nsegundo erro\r\n`, then ⌘F):
9. ⌘F in terminal → one FindBar, its field focused.
10. Type `erro` (after 150 ms) → 2 highlights, `2 de 2`; Enter → `1 de 2`; Shift+Enter → `2 de 2`.
11. Type `inexistente` → `Nenhum`, no highlights.
12. Type `erro`, then terminal writes `terceiro erro\r\n`, 300 ms → 3 highlights, counter `2 de 3` (current not moved).
13. Type `erro`, Esc → no FindBar, no highlights; then Enter goes to the pty (`\r` written).
14. Focus in an outside text field, ⌘F → FindBar appears in the focused pane with its field focused.
Plus sidebar_toggle_test: sidebar search now triggered by ⌘⇧F.

#### Rust landing

- `crates/mx-term/src/find.rs` (new): `find_in_grid(rows, width, query, limit) -> Vec<Hit>` pure + unit tests 1–6. (alacritty has `RegexSearch`/`RegexIter`; avoid it — smart-case literal + wide-char/column mapping semantics must match; use `regex::escape` + `RegexBuilder::case_insensitive` or plain lowercase compare.)
- `crates/mx-term/src/session.rs`: `find(&self, query) -> Vec<Hit>` reading the grid under the lock; hits as absolute line indices (history + screen) so they can be re-anchored — store hits as `(grid_line_from_top_of_history, col)` and re-derive on `refresh` using total-lines delta (alacritty has no line anchors; emulate by tracking `history_size` changes: when scrollback trims, shift).
- `crates/mx-app/src/find_bar.rs` (new, beside `terminal_view.rs`): `FindSession` + bar view (debounces 100/250 ms, counter, buttons, tooltips).
- `crates/mx-app/src/terminal_view.rs` / `terminal_element.rs`: overlay the bar, paint hit quads (yellow 0.24 / 0.62) in the overlay pass, scroll-to-centre (`display_offset` adjust), focus routing (Enter/Shift+Enter/Esc in the field, not to pty).
- `crates/mx-app/src/keymap/action.rs`: add `Action::FindInPane` (`find`, `Buscar no painel`, hint above, group navigation, `cmd(c('f'))` / Linux `ctrl_shift(c('f'))`); change `Action::Search` to `cmd_shift(c('f'))` / Linux `ctrl_alt(c('f'))`. `keymap/dispatch.rs`: global route + banner `Nenhum terminal em foco pra buscar`. `chrome/demo/settings.rs` row for search shows `Ctrl+Shift+F` — update to `Ctrl+Alt+F` and add the find row. `keymap/tests.rs` for cases 7–8.

#### Already in Rust / discrepancies

- No find exists. `keymap/action.rs` lines 179/204 still bind `Search` to ⌘F / Ctrl+Shift+F (2.4) — will conflict with `FindInPane` and must change in the same package.
- `terminal_element.rs` already has an overlay pass for selection (accent @ 30%); hit highlights go in the same pass, under the selection.

---

### C4 Cross-reference only (owned by other areas)

- b043ae0 (sessions API, area F) adds to the session: `asksTrust` = regex `trust this folder|Do you trust the files` (case-insensitive) over `screenText()` (visible rows, code 0 → space, one `\n` per screen row, columns `< min(viewWidth, len)`); `enter()` (Enter key unless exited); `sendKeys(list, gap = 60 ms)` — items are names `up down left right enter escape tab space backspace` or `{text: string}` typed, unknown names ignored, stops if the session exits. Rust landing for those helpers: `crates/mx-term/src/session.rs` (or a new `mx-term/src/keys_api.rs`) — coordinate with area F so only one package edits `session.rs` for them.
- c707872 (area E): `pid` falls back to a test-only `fakePid`.
- 5c7af80 / 610086e (area E): pane header/EmptyPane changes in `terminal_pane.dart`.


## D — TASK#82253: new widgets in the plugin interface (commit a61b94e)

Source of truth: `origin/main` versions of `lib/ui/plugin_editor.dart`, `plugin_input.dart`, `plugin_sprite.dart`,
`plugin_zoom.dart`, `plugin_selection.dart`, `plugin_modal.dart`, `plugin_rfw.dart`, `plugin_pane.dart`,
`doc_pane.dart`, `services/plugins.dart`, `services/plugin_api.dart`, `services/store.dart`, `docs/plugins.md`.
Baseline: spec 04 §rfw (2.4 had `Svg`, `Glyph`, `IconButton`, `Tooltip`, `Pressable`, `MenuButton`, `Draggable`,
`DropTarget` in `import maestria;`, `initialize.rfw: 2`).
Note: `plugin_input_test.dart` cases `terminal.pasteClipboard` and `openClaude(place: false)` were added by
b043ae0 (TASK#82254, sessions API) — they belong to that section, not here. `result_strip.dart` is not touched
by a61b94e.

Every new widget lives in `import maestria;` (`LocalLibrary::Maestria`). Event handlers follow the rfw rule:
`event "name" {args}` → `view.action` with `action: "name"` and `values` = the event's own args merged with the
extra map listed below (extra keys win).

---

### TASK#82253.0 — capability flags, `view.open` params, data-only redraw

#### Behaviour / Protocol
- `initialize` params gain (all value `1`): `editor`, `sprites`, `zoom`, `textInput`, `markdown`, `arrows`,
  `modalFrom`, `selectable`. (`rfw: 2`, `modal: 1` unchanged.) Doc also mentions `pasteImage: 1` but
  **plugins.dart does not send it** — do not send it either (or decide; flag as doc/impl mismatch).
- `view.open` gains optional `from: string` (Anchor id), `width: number`, `height: number` (`num → double`).
  Stored on the `PluginView` as `modalFrom`, `modalWidth`, `modalHeight`. When the view is already the open
  modal (same plugin + viewId) only `modalFrom = from ?? previous` is updated (width/height kept).
  On a fresh open all three are set (null clears).
- `PluginView.rfwChanges` (a per-view change counter) bumps on every `setRfw`. The store's global
  `notifyListeners` is now **skipped** for data-only updates:
  - `view.update`: notify only if `title` changed (`title != null && title != view.title`) or "switched"
    (`blocks != null` or (`rfw.library != null` and the view previously had no library)).
  - `sidebar.update`: notify only if the tab is new, switched (same rule) or the badge actually changed
    (`badge==""` and a badge existed, or badge differs).
- The rfw host keeps `_pushed: key → value` and only re-feeds data keys whose value instance changed
  (`theme` key is host-owned and skipped). Library re-parsed only when the text changed.
- The rfw root is wrapped so `SingleChildScrollView(primary: true)` does not share a global scroll controller
  (two rfw windows on screen — sidebar tab + modal — must both scroll without errors).
- `Svg` is now wrapped in its own repaint layer (`RepaintBoundary`).
- Docs text added (verbatim PT, informative): "Uma atualização só de dados redesenha só aquela janela (o resto do
  app nem fica sabendo), e só o que depende das chaves que vieram -- então mande só as que mudaram".

#### Tests (given/when/then)
- perf.1: given a store with an rfw view (`Stack[Svg(data.scene), Text(data.label), Sprite(...)]`) and a store
  listener counter; when `updatePluginView(rfw: data {label: "dois"})` returns true; then the window shows
  "dois", the listener count is 0, and the Svg widget instance is not rebuilt. When `updatePluginView(title:
  "outra")`; then listener count is 1.
- perf.2: given the same library rendered; then the Svg has a RepaintBoundary ancestor, and a Sprite whose sheet
  failed to load has no RepaintBoundary inside it (nothing to paint).
- scroll.1: given two rfw panes side by side (300×500 each) each with `SingleChildScrollView` of 40 rows of 60px;
  when the mouse hovers row 2 and scrolls 300px; then no exception after 100 ms and after 2 s.

#### Rust landing
- `crates/mx-plugins/src/registry/lifecycle.rs` (`initialize_params`: add the 7 missing flags).
- `crates/mx-plugins/src/rpc/api.rs` (`ViewOpen { from: Option<String>, width: Option<f64>, height: Option<f64> }`),
  `crates/mx-plugins/src/rpc/dispatch.rs` (`view.open` parse).
- `crates/mx-store/src/plugins/views.rs` (view model fields `modal_from/modal_width/modal_height`; notify rules).
- `crates/mx-app/src/rfw_view/view.rs` (`merge_data` should only re-evaluate dependents of changed keys; per-view
  notify instead of store-wide).

#### Already in Rust / discrepancies
- `initialize` sends `editor: 1` (already) but not `sprites`, `zoom`, `textInput`, `markdown`, `arrows`,
  `modalFrom`, `selectable`.
- `ViewOpen` has no `from/width/height`.
- gpui has no global primary scroll controller → scroll.1 is satisfied by construction; keep the test.
- Data-only update routing: verify whether a `view.update` with only `data` re-renders the whole app (store notify)
  in Rust; spec requires it to touch only that `RfwView` entity.

---

### TASK#82253.1 — `CodeEditor`

#### Behaviour
Args (all optional except `doc`):
| arg | type | default | notes |
|---|---|---|---|
| `doc` | string | `""` | buffer key (usually abs path) |
| `text` | string | — | read only when `doc` first appears in this window, or with a new `revision` |
| `revision` | int | 0 | new value + non-null `text` replaces the buffer text |
| `language` | string | null | null or unknown name → no highlighting |
| `line`, `column` | int (1-based) | null | target of a `jump` |
| `jump` | int | 0 | a new value (≠ last served) with `line != null` moves the cursor |
| `readOnly` | bool | false | |
| `wordWrap` | bool | false | |
| `lineNumbers` | bool | **true** (`!= false`) | gutter with numbers + fold (chunk) indicator |
| `fontSize` | double or int | 13 | |
| `autofocus` | bool | **true** (`!= false`) | |
| `hint` | string | null | placeholder when empty |
| `onChange`, `onSave`, `onCursor` | handlers | | |

- **Buffers per window, not per widget**: `Map<doc, EditorBuffer>` attached to the `PluginView` (lives as long as
  the window/tab is open). Buffer = text controller (with undo), `revision`, last served `jump`, `sent` (the text
  the plugin already knows), scroll `top`/`left`. Unmounting (panel off-screen, switching to sessions) keeps it;
  switching `doc` in the same widget detaches and attaches the other buffer.
- Attach: create buffer with `text ?? ''` and `revision` if absent; restore scroll offsets; focus after first frame
  if `autofocus` (the code requests focus only when `autofocus`, even for fresh docs).
- Sync (on attach and every update), applied post-frame:
  - `reload = spec.revision != buffer.revision && spec.text != null`; `jump = spec.line != null && spec.jump !=
    buffer.jump`. Neither → nothing. Set `buffer.revision = spec.revision`; if jump set `buffer.jump`.
  - reload and text differs: cancel pending change timer; remember cursor line index; set text; `sent = text`
    (so it is NOT echoed back as `change`); if no jump, put cursor at col 0 of the remembered line
    (clamped), make visible (not centered).
  - jump: cursor to `(line-1, (column ?? 1)-1)` clamped to line count / line length; center if invisible;
    request focus.
- `onChange`: debounce **350 ms** after the last text change; also flushed immediately on detach (doc switch,
  widget unmount) and before `onSave`. Payload extra `{doc, text}` (full text). Only when text identity differs
  from `sent`; text that came from the plugin never goes back.
- `onSave`: ⌘S (CodeShortcutSaveIntent; Ctrl+S off-mac) → flush change first, then `{doc, text}` immediately.
- `onCursor`: debounce **120 ms** after selection changes; extra `{doc, line: extentIndex+1, column:
  extentOffset+1, selected: collapsed ? 0 : selectedText.length}`. Fires also after a jump/reload moves the cursor.
- Keyboard (re_editor defaults + overrides): ⌘F find, ⌘⌥F find+replace (re_editor: Ctrl+H on Linux/Windows),
  ⌘/ toggle line comment (per-language formatter below), ⌘Z/⌘⇧Z undo/redo, folding via gutter chunk indicator.
- Rendering: bg `Mx.bg`; padding L6 R10 T6 B40; font `Mx.mono`, size `fontSize`, line height 1.5; text `fg`;
  hint `fgFaint`; selection `accent @0.28`; search highlight `yellow @0.30`; cursor `accent`; current-line fill
  `fg @0.045`; chunk indicator `fgFaint` width 18. Line numbers mono `fontSize-1` `fgFaint`, focused line `fgDim`.
- Context menu (right-click) uses the app menu: `Recortar ⌘X` (enabled if selection), `Copiar ⌘C` (if selection),
  `Colar ⌘V`, divider, `Selecionar tudo ⌘A` (chord prefix `⌘` on macOS, `Ctrl+` else); icons content_cut,
  content_copy, content_paste, select_all (14px `fgDim`).
- Find bar (overlay, top-right, width 400, margin T6 R14 B6, padding 4, bg `bgSidebar`, radius 8, border `border`,
  shadow `shadow` blur 14 offset (0,4)). Row 1: toggle-mode icon (chevron_right / expand_more when replace open;
  tooltip `trocar`; disabled when readOnly) · field hint `buscar` with trailing toggles `Aa` (tooltip
  `maiúsculas`, case sensitive) and `.*` (tooltip `expressão regular`) · counter (width 64, 11.5px): empty when
  pattern empty, `nenhum` (red) when no match, else `"{i+1} de {n}"` (`fgDim`) · `anterior` (arrow_upward) ·
  `próximo` (arrow_downward) · `fechar` (close). Row 2 (replace mode, not readOnly): indent 26 · field hint
  `trocar por` · `trocar este` (find_replace) · `trocar todos` (done_all) · spacer 52. Fields: height 30, bg `bg`,
  radius 5, border, text 12.5 `fg`, hint `fgFaint`, Enter = next match (find) / replace one (replace) and keep focus,
  Esc closes. Toggle chips 24×22, on = accent@0.18 fill + accent@0.6 border, label 11 mono. Icon buttons 26×26,
  icon 16, `fgDim` (`fgFaint` disabled). Bar height = rows*30 + (rows-1)*4 + 6*2 + 8.
- Languages (name → comment): slashes `//` + `/* */`: dart javascript typescript json go rust java kotlin swift
  c cpp csharp objectivec php scss less protobuf gradle groovy scala; hash `#`: yaml python ruby bash powershell
  dockerfile makefile ini properties graphql elixir perl r nginx; dashes `--`: sql lua haskell; markup `<!-- -->`:
  markdown xml vue; css: block `/* */` only; diff, plaintext: none.
- Syntax colors from the active theme: root `fg`; comment/quote `fgFaint` italic; keyword/doctag/formula/
  selector-tag `ansi.magenta`; built_in/type/selector-class/class-title/selector-attr/selector-pseudo `ansi.yellow`;
  title/function `ansi.blue`; section `ansi.blue` w600; string/regexp/addition/meta-string/code `ansi.green`;
  number/literal/symbol/bullet `ansi.cyan`; attr/attribute/property `ansi.cyan`; params/subst `fg`;
  variable/template-variable/name/selector-id/deletion `ansi.red`; tag/meta `fgDim`; link blue underline;
  emphasis italic; strong bold. Class lookup: strip after first `.`, nested joined `outer-inner`, fall back by
  dropping prefixes, else `fg`.

#### Protocol
`initialize.editor: 1`. Events via `view.action` (`change`/`save`/`cursor` are whatever names the plugin gave).

#### Tests
- ed.1 (macOS): given doc `/repo/a.dart` text `void main() {}\n` rev 1; when rendered; then no `change` sent. When
  `// oi\n` is inserted at 0:0; then no `change` at +100 ms; at +500 ms one `change` with `doc=/repo/a.dart`,
  `text="// oi\nvoid main() {}\n"`. When the editor is clicked and ⌘S pressed; then a `save` with the same text.
- ed.2: given doc a (`a\n`, rev 1) rendered and the buffer changed to `a\nb\n`; when the pane is unmounted; then a
  `change` was sent immediately (no debounce wait). When remounted with the same data; then text is still
  `a\nb\n`. When data switches to doc b (`b\n`, rev1) then back to doc a (`a\n`, rev 1); then b shows `b\n` and a
  still shows `a\nb\n`.
- ed.3: given doc `um\n` rev1; when data becomes text `um\ndois\ntrês\n` rev 2 line 3 jump 1; then text replaced,
  cursor line index 2; after 500 ms no `change` was sent and the last `cursor` has `line=3`.
- ed.4: syntax keyword color = `ansi.magenta`, string = `ansi.green`; `codeLanguage("dart")` exists,
  `codeLanguage("klingon")` is null.

#### Rust landing
- `crates/mx-rfw/src/widgets/code_editor.rs` (parse `lineNumbers`, `autofocus`, `hint`; `language: Option`).
- `crates/mx-app/src/rfw_view/code_editor.rs` (spec, `editor_style` colors, buffer ownership per view),
  `crates/mx-app/src/rfw_view/code_pane.rs` (+ new `code_find_bar.rs` for the overlay bar, `code_menu.rs` for the
  context menu), `crates/mx-app/src/rfw_view/code_pane_tests.rs`.
- Shared editor core `crates/mx-app/src/setup/editor/{keys.rs,syntax.rs,element.rs}` (⌘/ comment, fold gutter,
  current-line fill, cursor centering) — **shared with the setup editor**; give this to one owner.

#### Already in Rust / discrepancies (crates/mx-app/src/rfw_view/code_editor.rs, code_pane.rs, mx-rfw code_editor.rs)
Implemented: buffer per `doc` (entity map in `CodeEditors`), `text`/`revision`/`jump`/`line`/`column`,
`readOnly`, `wordWrap`, `fontSize` (int or double), 350 ms change debounce, save flushes pending change first,
cursor payload `{doc,line,column,selected}`, find (Ctrl/⌘F), replace (Ctrl/⌘H), next/prev (⌘G/⇧⌘G, F3, Enter),
replace one/all, Esc closes, undo/redo (500 steps), smart-home, auto-indent on Enter, 39-name grammar table.
Discrepancies:
1. **Data loss bug**: `CodePane::apply` reloads when `reappeared && !waiting && text != buffer` — Dart never
   reloads on reappearance with the same `revision`. ed.2 (doc a → b → a with `text "a\n"` rev 1) would wipe the
   typed `a\nb\n` in Rust. Reload must be `revision changed && text.is_some()` only.
2. Reload does not preserve the cursor line (Dart keeps the line index, col 0) — check `TextEditor::reload`.
3. No flush on detach: Dart sends `change` immediately when the doc leaves the screen/unmounts; Rust waits for
   the 350 ms timer (and a pane that is no longer shown still fires later).
4. Buffer lifetime: Rust buffers live in `RfwView.editors`, owned by `plugin_ui::Surface`; docking a modal
   (`Surface::new` in `panes/bodies.rs`) or pruning a surface loses buffers. Dart ties them to the `PluginView`
   (tab) — move the map to a per-tab owner that survives surface recreation.
5. Missing args: `lineNumbers` (Rust always shows gutter), `autofocus` (Rust always focuses on create and on every
   reappearance), `hint`.
6. `language` default: Rust `"plaintext"`; Dart null. Unknown names must render uncolored (check `syntax.rs`
   fallback).
7. Cursor event: Rust fires on every `EditorEvent::Cursor` deduped; Dart debounces 120 ms.
8. Syntax colors differ: Rust string `theme.green`, number `theme.yellow`, keyword `theme.purple`, kind
   `ansi.cyan`, key `ansi.blue`, meta `accent`. Dart: keyword `ansi.magenta`, type `ansi.yellow`, function
   `ansi.blue`, string `ansi.green`, number `ansi.cyan`, variable `ansi.red`, tag/meta `fgDim`, comment italic.
9. Selection alpha 0.30 vs 0.28; find highlight 0.22/0.50 vs a single `yellow@0.30`; no current-line fill.
10. Find bar: Rust is a full-width strip above the editor; Dart a 400px floating card top-right. Rust count
    strings `sem resultados` / bare number; Dart `nenhum` (red) / `"i de n"`. Rust has no `Aa` case toggle (uses
    smart-case auto) and no `.*` regex toggle; no `trocar` mode toggle chevron; replace buttons are text
    (`trocar`, `trocar todos`) instead of icons with tooltips `trocar este`, `trocar todos`; no tooltips
    `anterior`/`próximo`/`fechar`. Height 24 vs 30.
11. Replace shortcut: Dart ⌘⌥F on macOS (Ctrl+H elsewhere); Rust Ctrl/⌘H everywhere. Add ⌘⌥F on macOS.
12. Missing: ⌘/ comment toggle, code folding (chunk indicator), right-click context menu
    (`Recortar`/`Copiar`/`Colar`/`Selecionar tudo`).
13. Jump does not center the cursor when off-screen (verify `go_to`).
14. Padding (L6 R10 T6 B40) and line-number font (`fontSize-1`, focused `fgDim`) not verified.
15. Scroll offset per buffer: kept only while the entity lives (see 4).

---

### TASK#82253.2 — `TextInput`

#### Behaviour
Args: `id` (string, `""`), `placeholder?`, `value?` (read only when `id` first appears in the window), `hint?`,
`multiline` (false), `maxLines` (int or double rounded, 6; only with multiline), `autofocus` (false),
`disabled` (false), `clear` (int|double→int, null), `sendIcon` (plugin icon name), `fontSize` (double, 13),
`allowEmpty` (false), `commands` (`[{name, description?}]`; names trimmed, leading `/` stripped, blank names
dropped), handlers `onSubmit`, `onPasteImage`, `onArrow`.
- Text lives in the window: `Map<id, controller>` per `PluginView`. `clear`: per-view map `id → last clear seen`;
  when the id was seen before and `clear` differs → empty the field; then record. First appearance never clears.
  Same data again (other keys changed) keeps typed text.
- Submit: if disabled → nothing; `text = controller.text.trim()`; empty and !allowEmpty → nothing; else
  `onSubmit` extra `{text, id}`. **The field is NOT cleared on submit** — only the plugin clears it via `clear`.
  Single-line: Enter submits. Multiline: Enter submits, Shift+Enter inserts newline.
- `sendIcon`: suffix IconButton, tooltip `enviar (enter)`, icon size 17, color `accent` (`fgFaint` when disabled),
  disabled → no action.
- `disabled`: field not editable; hint text = `hint ?? placeholder` (otherwise `placeholder`).
- Paste: only when `onPasteImage` is set, ⌘V (Ctrl+V off-mac) is intercepted: clipboard text non-empty → insert at
  selection (or end), caret after; no text (an image) → `onPasteImage` extra `{id}`, text untouched. Disabled →
  nothing.
- `onArrow`: when set, not disabled, key-down (not up), field text empty: ArrowUp/ArrowDown → extra
  `{key: "up"|"down", id}` and the key is consumed. With text, arrows belong to the field.
- Slash commands: list shown when `commands` non-empty, not disabled, text matches `^/(\S*)$` and text ≠ the text
  at which Esc dismissed. Filter q = lowercased capture: names starting with q first, then names containing q, max
  **8**. While open: Up/Down move highlight (wrap), Tab completes highlighted → text `"/name "` caret at end,
  Enter (no shift) sets text `"/name"` and submits, Esc dismisses until text changes, click completes. Highlight
  resets to 0 on every text change. The plugin learns nothing until submit.
- Rendering: text `fontSize` `fg` height 1.35; filled `bgActive`; hint `fgFaint` size `fontSize-0.5`; padding
  L12 T10 R8 B10; radius 10; border `border`; focused border `accent` width 1.4. Command list above the field
  (margin-bottom 6, padding V4, bg `bgSidebar`, border, radius 10); rows: padding H4 outer, inner L10 T5 R10 B5,
  radius 7, highlighted bg `bgActive`, hover `bgHover`; `/name` 12.5 w600 (`accent` when highlighted else `fg`),
  min width 140, gap 12, description 12 `fgFaint` single-line ellipsis.

#### Protocol
`initialize.textInput: 1`, `arrows: 1` (no `pasteImage` flag is actually sent).

#### Tests
- in.1 (macOS): given `TextInput(id:"chat", multiline, clear: data.sent, disabled: data.locked, hint:
  "responda no terminal", sendIcon:"send")` with sent 0; when "roda os testes" typed + Enter; then one `send`
  with `text="roda os testes"` and the field still holds it. When data sent=1; then field empty. When field =
  "meio caminho" and data updated with sent still 1; then text kept. When field = "   " and send button tapped;
  then nothing sent.
- in.2: given locked=true; then "responda no terminal" is shown; with text "sim", tapping send sends nothing.
- in.3 (macOS): given `onPasteImage`, `allowEmpty: data.empty=false`; clipboard "olha isso" + ⌘V → field
  "olha isso", no action; clipboard without text + ⌘V → `image` action, text unchanged; field "" + Enter → nothing;
  data empty=true + Enter → `send` with `text=""`.
- in.4: given `onArrow`, empty field focused; Down, Down, Up → `["down","down","up"]`; after typing "oi", Down → no
  new event.
- in.5 (macOS): commands clear/compact/context, multiline; type `/co` → shows `/compact`, `/context`, not `/clear`;
  Down + Tab → text `/context ` and list closed; type `/comp` + Enter → submitted `/compact`; type `/c` + Esc →
  list hidden; type `/cl` → `/clear` shown; click it → text `/clear `.

#### Rust landing
- New `crates/mx-rfw/src/widgets/text_input.rs` (arg parsing) + register in `crates/mx-rfw/src/tree.rs`.
- New `crates/mx-app/src/rfw_view/text_input_widget.rs` (widget, per-view state map, slash list) — keep the
  existing primitive `text_input.rs` untouched (used by blocks and code_pane); add a dispatch line in
  `widgets_maestria.rs`.

#### Already in Rust / discrepancies
None: `TextInput` is not in the Maestria widget list in `mx-rfw/src/tree.rs`. The primitive
`rfw_view/text_input.rs` (single-line, placeholder, size) can be the editing core but lacks multiline, selection
replace on paste, image paste detection, arrow interception.

---

### TASK#82253.3 — `Markdown` and markdown selection

#### Behaviour
- `Markdown(text, fontSize?)` → the reader-pane markdown renderer (`MxMarkdown`), same stylesheet, GFM
  (tables, task lists, strikethrough), default size `MxMarkdown.baseSize`; links → `store.followLink(href,
  from: tab)` (same as reader).
- Change to `MxMarkdown` everywhere (reader pane too): no per-paragraph selectable text; the whole markdown body
  is one selection area; inside an outer selection container (a `Selectable`) it joins that one.

#### Tests
- md.1: `Markdown(text: "Pronto, o **login** voltou.\n\n- um\n- dois")` → no text containing `**`; "login" and
  "dois" rendered.

#### Rust landing
New `crates/mx-app/src/rfw_view/markdown_widget.rs` reusing `crates/mx-app/src/reader/` renderer; parse in
`crates/mx-rfw/src/widgets/maestria.rs`; dispatch in `widgets_maestria.rs`. Reader selection change in
`crates/mx-app/src/reader/` (owner of the reader).

#### Already in Rust / discrepancies
Not present as an rfw widget (blocks have `Block::Markdown`, rendered in `blocks_misc.rs`). Check whether the Rust
reader selects across paragraphs.

---

### TASK#82253.4 — `Selectable` and `SelectionLines`

#### Behaviour
- `Selectable(child)`: one selection area for everything inside; drag crosses paragraphs/bubbles/messages; ⌘C copies.
- Copied text joining: walk selected pieces in order; between consecutive pieces insert `"\n"` if
  `after.top >= before.bottom - 2` (global rects), else `" "` (side by side, e.g. list bullet and item text).
  Empty pieces skipped.
- `SelectionLines(child)`: the same joining for content inside a scroll area whose `Selectable` is outside (so a
  drag auto-scrolls).
- Markdown list bullets copy as `•`.

#### Protocol
`initialize.selectable: 1`.

#### Tests
- sel.1 (macOS): `Selectable(Column[Markdown("primeiro parágrafo\n\nsegundo parágrafo"), Text("no meio"),
  Markdown("- um item\n- outro item")])`; no per-paragraph selectable widgets; drag from "primeiro" to the end of
  "outro item" + ⌘C → clipboard contains `primeiro parágrafo\nsegundo parágrafo\nno meio\n` and `• um item\n• outro
  item`.

#### Rust landing
New `crates/mx-app/src/rfw_view/selectable.rs` (gpui needs a custom cross-element selection: collect text runs with
bounds, hit-test drag, join with the rule above); parse in `mx-rfw/src/widgets/maestria.rs`.

#### Already in Rust / discrepancies
Not present. Highest-effort item of this section in gpui (no built-in cross-widget selection).

---

### TASK#82253.5 — `Sprite`

#### Behaviour
Args (numbers accept int or double): `sheet` (path relative to the plugin dir), `columns` (1), `rows` (1), `row`
(0), `start` (0), `frames` (1), `fps` (6.0), `width?`, `height?` (screen size; default = cell size), `flip`
(false), `paused` (false), `loop` (true).
- File resolution `pluginFileOf(rel, dir)`: null if rel empty/null, dir null, rel contains `..`, absolute path
  outside dir, resolved path not under `dir/`, or file missing.
- Frame at elapsed t: `frames<=1 || fps<=0 → 0`; `n = floor(t_us * fps / 1e6)`; loop → `n % frames`, else
  `clamp(n, 0, frames-1)` and the ticker stops at the last frame.
- Animates iff `!paused && frames>1 && fps>0`. Restart from frame 0 when any of `row,start,frames,fps,paused,loop`
  changes; same values again → continue from current frame. File change → reload image.
- Draw: cols/rows clamped ≥1; cell = image size / grid; source col = clamp(start+frame, 0, cols-1), row =
  clamp(row, 0, rows-1); nearest-neighbour (no smoothing, no AA); `flip` mirrors horizontally; own repaint layer.
  Before the image loads: an empty box of `width ?? 0` × `height ?? 0`. Load errors silent.

#### Protocol
`initialize.sprites: 1`.

#### Tests
- sp.1: fps4 frames4 → at 0,249,250,500,750,1000,1250 ms frames `0,0,1,2,3,0,1`.
- sp.2: fps4 frames3 loop=false at 9 s → 2.
- sp.3: frames1 fps8 at 3 s → 0; fps0 frames4 → 0.
- sp.4: `a.png` in dir → resolves to `…/a.png`; `../a.png`, `/etc/hosts`, `""`, `nao-existe.png`, and dir=null →
  null (a sibling dir `<dir>-irmao.png` must not match).
- sp.5: file null, columns4 rows2 frames4 fps10 16×24 → frame 0, size 16×24; after 10+250 ms frame 2; same widget
  again → still 2; row 1 → 0; row1 paused → after 500 ms still 0.

#### Rust landing
New `crates/mx-rfw/src/widgets/sprite.rs` (parse + pure `sprite_frame`, `plugin_file_of` with unit tests); new
`crates/mx-app/src/rfw_view/sprite.rs` (gpui `img` with nearest sampling / custom paint, `request_animation_frame`
timer per sprite).

#### Already in Rust / discrepancies
Not present. gpui image sampling: confirm nearest-neighbour is possible (otherwise pre-scale the cell with
`image` crate nearest filter and cache by (sheet, cell, size, flip)).

---

### TASK#82253.6 — `ZoomView`

#### Behaviour
Args: `child`, `maxScale` (number, default 6, min 1), `command` (`"in"|"out"|"fit"`), `commandRev` (int|double,
0), `onZoom`.
- Child sized exactly to the ZoomView box; scale 1 = whole child; min scale 1, max `maxScale`.
- Mouse wheel zooms around the cursor (Flutter `InteractiveViewer` with `scaleFactor: 200` ≈ 10% per notch;
  scroll delta −100 → scale > 1.2), drag pans, trackpad two-finger pans and pinch zooms. Translation clamped so no
  area outside the child shows.
- `command` runs only when `commandRev` changes after mount (initial value is ignored). `in` = scale×1.5, `out` =
  scale/1.5, `fit` = 1; clamped to [1, maxScale]; keeps the content point at the viewport centre fixed; animated
  **220 ms easeOutCubic**; user interaction start stops the animation. Target ≤1.0001 → identity.
- `onZoom` extra `{scale: double}` **200 ms** after the transform stops changing, only if |scale − last reported|
  ≥ 0.005 (initial reported = 1).

#### Protocol
`initialize.zoom: 1`.

#### Tests
- zm.1: 400×200 view maxScale 4; wheel delta (0,−100) at centre → scale > 1.2 immediately, no report yet; after
  250 ms exactly one report ≈ current scale.
- zm.2: wheel (0,+300) → scale stays 1.
- zm.3: initial command "in" rev 0 → scale 1; rev 1 → 1.5; revs 2..7 "in" → 4 (clamped); "fit" rev 8 → 1 and
  after 250 ms last report = 1.

#### Rust landing
New `crates/mx-rfw/src/widgets/zoom_view.rs` (parse) and `crates/mx-app/src/rfw_view/zoom_view.rs` (state entity
keyed by widget path: matrix, animation, settle timer; pure math `zoom_target`, `clamp_translation` unit-tested).

#### Already in Rust / discrepancies
Not present.

---

### TASK#82253.7 — `Anchor` and modal grow-from-anchor (`view.open` `from`/`width`/`height`)

#### Behaviour
- `Anchor(id, child)`: registers the child's on-screen rect under key `"{pluginId}/{id}"` (global registry; last
  mounted wins; unregister on dispose only if still owned). Rect is in window coordinates including ancestor
  transforms (e.g. inside a FittedBox).
- Modal layer (changed for every modal, anchor or not):
  - Size: `width = clamp(modalWidth ?? maxW-48, 320, clamp(maxW-48, 320, 820))`; `height = clamp(modalHeight ??
    maxH-48, 240, maxH*0.88)`; centred.
  - Open animation **340 ms easeOutCubic**, close **260 ms easeInCubic** (reverse). Start rect = anchor rect (read
    at open) or, without anchor/anchor off-screen, the end rect scaled 0.97. Scale lerps `start.w/end.w → 1`
    (clamped 0.02..1), centre lerps start→end; opacity `clamp(t*1.6, 0, 1)`; backdrop black `0.42*t`.
  - Closing (×, Esc, backdrop click, `view.close`) keeps the modal rendered until the reverse animation ends; the
    anchor rect is re-read at close (the anchor may have moved). Docking ("abrir como painel") does not animate.
  - Header changed: title+subtitle in an Expanded row (title Flexible, gap 8, subtitle Flexible flex 2, 12px
    `fgDim`); "abrir como painel" is now an **icon button** (vertical_split_outlined, 15px, 26×26, tooltip `abrir
    como painel`) instead of a text button; close button 26×26 (was 28), tooltip `fechar (esc)`.
  - Docs: "Sem `from`, ou com a âncora fora da tela, ele aparece no meio."

#### Protocol
`view.open { …, modal: true, from?, width?, height? }`; `initialize.modalFrom: 1`.

#### Tests
- mo.1: surface 1200×900, anchor `wiboor/agente:1` at (100,700) 20×30 → `pluginAnchorRect` = exactly that rect.
  `openPluginView(modal, from:"agente:1", width 500, height 400)`; first frame body width < 40 and centre within
  30px of (110,715); settled centre x ≈ 600 (±2), width ≈ 500 (±2). `closeModal` → after 150 ms body still present,
  narrower than full, centre y below the full centre y; settled → body gone.
- mo.2 (changed): docking is triggered via `find.byTooltip('abrir como painel')`.

#### Rust landing
New `crates/mx-app/src/rfw_view/anchor.rs` (registry: `HashMap<String, Bounds>` updated during prepaint) ; modal
animation in `crates/mx-app/src/plugin_ui/modal.rs`; fields in `crates/mx-store/src/plugins/views.rs`;
params in mx-plugins (see .0).

#### Already in Rust / discrepancies
`plugin_ui/modal.rs` exists with a text button `DOCK` ("abrir como painel" as text) — must become an icon button
with tooltip; no open/close animation, no keep-alive while closing, no requested size.

---

### TASK#82253.8 — blocks `list` additions and icon table

#### Behaviour
- List item `avatar` object gains `size` (number, clamped 10..40) replacing the default (26 flat / 30 other); when
  custom, the gap after the avatar is 7 instead of 10.
- List item `titleTone`: title color = tone (selected still wins with `onAccent`; then `titleTone`; then dim → `fgDim`;
  else `fg`).
- `pluginIcon` new names: `clean` → cleaning_services_outlined, `send` → send_rounded, `chat` →
  chat_bubble_outline_rounded, `pets` → pets_rounded, `photo` → image_outlined, `home` → home_outlined, `moon` →
  bedtime_outlined, `shield` → verified_user_outlined, `close` → close_rounded, `palette` → palette_outlined.
  (`clear` stays block_rounded.)

#### Tests
- ic.1: `pluginIcon('clean')` = cleaning_services_outlined; `pluginIcon('clear')` = block_rounded.
- li.1: flat list `[{title "app.js", avatar {svg, size 16}, titleTone "yellow"}, {title "docker", avatar {svg}}]`
  → first svg 16×16, second 26×26; "app.js" color `yellow`, "docker" color `fg`.

#### Rust landing
`crates/mx-plugins/src/ui/icons.rs` (name map), `crates/mx-app/src/rfw_view/icon_paths.rs` (10 new Material
glyph paths), `crates/mx-plugins/src/ui/blocks.rs` (`Avatar.size`, `title_tone`),
`crates/mx-app/src/rfw_view/blocks_list.rs` (size + gap + title color).

#### Already in Rust / discrepancies
Missing all 10 icons, `avatar.size`, `titleTone`. (Also noticed: the Rust icon map has no `box` entry although
docs list it — outside this section, verify.)

---


## E — App-shell UI delta (hover, panel hotkeys, pane drag, session cards, history attach, switch, folders picker)

Source commits (Flutter `origin/main`): e5101c7 (TASK#80567), 5c7af80 (TASK#82250), c707872 (TASK#82252), 610086e (TASK#82249), 281c232 (TASK#82254, folders picker). Baselines: spec 01 (store/config), 05 (UI/menus/settings/keymap).

Hunks in the files of this area that belong to other areas (do not implement here):
- `dialogs.dart` e570922: terminal context-menu "copy" calls `copySelection(reflow: tab.kind == claude)` → area C (terminal/BUG#80709).
- `doc_pane.dart` a61b94e: `MxMarkdown` is no longer per-paragraph selectable; wraps in one `SelectionArea` unless already inside a `SelectionContainer` → area D (plugin widgets).
- `settings.dart` b043ae0: plugin row shows `PluginKindTag` per manifest `kinds` instead of the state label text → area F. d77be94/4cafe46: "Atualizações"/plugin auto-update sections → areas A/B (only the switch restyle below is ours).
- `sidebar_rail.dart` e9c30be: rail "+" opens `showPluginCatalog` instead of `showInstallPlugin` → area B.
- `notices.dart` 4cafe46: update chip → area A.

---

### TASK#80567 — Rounded inset hover on rows and menus (e5101c7)

#### Behaviour
Every hover highlight becomes a rounded card inset from the container edges, instead of an edge-to-edge rectangle. The margin sits *outside* the painted hover (fixes hover bleeding around rounded session cards).

New primitive `MxHover` (panel.dart): `Padding(margin) > InkWell(borderRadius: radius, hoverColor: hoverColor ?? Mx.bgHover)`. Defaults: `margin = horizontal 7`, `radius = 9`. `hoverColor: transparent` = row paints its own lit background and only wants click/ripple clipped to the same shape. Props: `onTap`, `onSecondaryTapDown`, `onHover(bool)`.

Exact per-site values (margin / radius / inner padding LTRB):

| Site | Margin | Radius | Inner padding (old → new) | hoverColor |
|---|---|---|---|---|
| Popup menu rows (`mxItem`, submenu opener, submenu rows) | horizontal `_inset = 5` | 6 | horizontal `_pad` → `_pad − 5` (label does not move) | bgHover (ink clipped by transparent `Material` with `RoundedRectangleBorder(6)`, `Clip.antiAlias`) |
| Filter sheet "clear filters" row | LTRB 5,0,5,3 | 6 | 12,8,12,10 → 7,8,7,7 | bgHover |
| Filter sheet `_FilterRow` | horizontal 5 | 6 | (10+indent),6,12,6 → (5+indent),6,7,6 | bgHover (BoxDecoration) |
| History "more chats" `_MoreChats` | horizontal 6 | 7 | 10,7,12,7 → 4,7,6,7 | bgHover |
| History `_ChatRow` | horizontal 6 | 7 | 10,7,12,7 → 4,7,6,7 | bgHover |
| Notice list row (bell list, not toast) | horizontal 6, vertical 3 | 7 | 12,10,6,10 → 6,7,0,7 | bgHover |
| Notice toast | none | none (card clips itself) | 12,10,6,10 unchanged | bgHover |
| Quick pick rows (plugin `_QuickPick`) | horizontal 6 (moved from inner Container to MxHover) | 6 | 10/7 unchanged; lit row `bgActive` | transparent |
| Result strip `_FileRow` | horizontal 6 | 6 | h12 v4 → h6 v4 | bgHover |
| Settings shortcut cap "×" (`_Cap`) | — | InkWell radius 4 | — | — |
| Sidebar folder header (`_FolderGroup`) | default (h7) | 9 | 10,8,8,8 → 3,8,3,8 | bgHover |
| Sidebar "Grupos" tray header | left 2, right 7 (outer top 4 moved outside) | 9 | left 4,right 8,top 4 → left 2,right 1 | bgHover |
| Sidebar feature/hotfix header | horizontal 4 | 9 | 7,8,4,8 → 3,8,3,8 | bgHover |
| Sidebar session row `_Row` | horizontal 7, vertical 3 (moved from Container margin) | 9 | 11,11,9,11 unchanged; card radius 9 | transparent (card paints bg) |
| Sidebar workspace header | default (h7) | 9 | 10,8,8,8 → 3,8,3,8 | bgHover |

Invariant: icon/text x-positions are unchanged — inset is subtracted from the inner left padding.

#### config.json / shortcuts
None.

#### Tests
None added (visual). `test/menus_test.dart` touched by a later commit only.

#### Rust landing
- `crates/mx-app/src/chrome/menu/row.rs`: wrap row in `mx(5)` + `rounded(6)` hover; padding `14 → 9` horizontal (`_pad` is 14 in Rust).
- New helper `crates/mx-app/src/chrome/widgets/hover.rs` (`MxHover`-like: outer margin, inner `rounded(r)` + `hover(bg_hover)`), exported via `chrome/widgets/mod.rs`.
- Sites: `crates/mx-app/src/sidebar/tree.rs` (folder/workspace/feature headers), `sidebar/trays.rs` (Grupos tray), `sidebar/tab_row.rs` (already `mx(7)`), `crates/mx-app/src/chrome/notices/*` (bell row), `crates/mx-app/src/app/chats.rs` (history rows, filter sheet rows), `crates/mx-app/src/plugin_ui/pick.rs` (quick pick), result-strip file rows (wherever they live under `panes/`).

#### Already in Rust
`chrome/menu/row.rs` paints edge-to-edge `hover(bg_hover)` with `pl(14+indent) pr(14)` — 2.4 behaviour. `tab_row.rs` already has `mx(7)` card margin; verify hover is not painted outside the card.

---

### TASK#82250 — Per-panel hotkeys (replaces ⌘1…⌘9 / Alt+1…9) (5c7af80)

#### Behaviour
- **Removed**: the fixed slot keys (`MxChord.slot(i)`: ⌘1–⌘9 on macOS, Alt+1–Alt+9 on Linux → select the i-th tab of `store.tabs`), `AppStore.selectIndex`, `MxKeys.digits`, the "Fixed" shortcuts-page row `'⌘1 … ⌘9' — 'Ir pra enésima sessão da lateral'`, the empty-screen cheat sheet line `'⌘1…9' 'Ir pra enésima sessão'`, and the recorder rejection `'é a tecla da Nª sessão da lateral'`. ⌘1–⌘9 / Alt+1–9 are now ordinary recordable chords (`chord.rejection == null`).
- **Added**: `MxTab.hotkey: MxChord?` (default null). No tab gets a key by position.
- Panel context menu (sidebar row, pane header, session card — `showPanelMenu`) gets a new item right after "Renomear…"/tint items and before plan/message/chain: id `hotkey`, glyph `Icons.keyboard_outlined` 14 fgDim, label `'Definir atalho…'` when none, `'Trocar atalho…'` when set, trailing chord label = current hotkey label.
- Dialog `showPanelHotkey` (AlertDialog, bg `Mx.bgSidebar`, content width 320):
  - Title: `'Atalho de "<tab.title>"'` (font 15).
  - Body text (12, fgDim): none → `'A combinação que traz este painel pra tela, de onde você estiver.'`; set → `'Hoje é <label>. Pressione outra pra trocar.'`.
  - `ShortcutRecorder` (the settings recorder, made public; Esc/cancel closes dialog).
  - On capture, rejection reason, in order: `chord.rejection` (existing recorder rules) → if keymap action owns it: `'já é o atalho de "<action.label>"'` → if a plugin command has that key: `'já é a tecla de "<command.title>", de um plugin'`. Shown below as `'<label>: <why>'` (11, `Mx.red`), dialog stays open.
  - Actions: `'Remover atalho'` (red, only when current != null → sets null), `'Cancelar'`.
  - Another panel's key is **stolen** (not rejected).
- `AppStore.setHotkey(tab, chord?)`: for every other tab with the same chord → clear it and `showBanner('<label> saiu de "<other.title>"')`; set; save; notify.
- `AppStore.bindShortcut(action, chord)` (settings): additionally clears any tab whose hotkey == chord with banner `'<label> saiu do painel "<tab.title>"'`.
- Dispatch order: app actions → panel hotkeys → plugin commands. Window level: `MxKeys.bindings` adds `hotkey.activator → store.select(tab)` after the action entries (first match wins). Inside the terminal key handler: keymap action → `MxKeys.panelFor(store, event)` (first tab whose `hotkey.accepts(event)`) → `select(target)`, handled → plugin `commandFor`.
- Display: `PaneKeyHint(chord)` in every pane header (terminal/doc/setup/plugin): renders nothing when null; else `Padding(left 8, right 2)` text label, mono 10.5 fgFaint. Sidebar tab row trailing shows `hotkey.label` (mono 10.5 fgFaint, right pad 6) only when set (was: index < 9).
- `select(tab)` semantics unchanged (opens/focuses).

#### config.json
- Per-pane layout entry (`panes[]` in the window layout, spec 01 §2.2 tab JSON): new optional key `"hotkey": "<MxChord.id>"` written only when set. **Not** in `MxTab.recipe` (pane-group recipes): a group re-opening would duplicate the key.
- Load: `MxChord.parse(pane['hotkey'])`; kept only if non-null, `valid`, and `keymap.owner(chord) == null` (an app action that claimed the key since then wins). Plugin-command collisions are not checked on load.
- Back-compat: absent key → null. 2.4 readers ignore it.

#### Shortcuts
Slot chords removed from `MxChord` and keymap fixed list; nothing replaces them by default.

#### Tests (panel_hotkey_test, pane_header_test, shortcuts_test, typography_test)
- Given a new tab, then `hotkey == null` and window bindings contain no Digit1 trigger.
- Given chords ⌘1…⌘9 (meta+digit), then `rejection == null` for each.
- Given tabs A ("primeiro"), B; when `setHotkey(A, ⌥⌘G)` then `setHotkey(B, ⌥⌘G)`, then A.hotkey null, B.hotkey ⌥⌘G, banner contains "primeiro".
- Given A with ⌥⌘G; when `setHotkey(A, null)`, then A.hotkey null.
- Given A with ⌥⌘G; when `bindShortcut(openMarkdown, ⌥⌘G)`, then A.hotkey null and banner contains "primeiro".
- Given A with ⌥⌘G, then `A.toJson()['hotkey'] == chord.id`, `parse(id) == chord`, and `A.recipe` has no `hotkey`.
- Given tabs A, B; B has ⌥⌘G; then window bindings include a KeyG trigger.
- Pane header: tab with ⌥⌘G (alt+meta+G) → `PaneKeyHint` shows `'⌥⌘G'`; tab without → `PaneKeyHint` renders no text (no positional `⌘1`).
- shortcuts_test: `MxChord(digit1, alt).label == 'Alt+1'`; "no default collides with copy or paste" (slot set dropped from the fixed set).
- typography_test: `keymap.owner(⌘0) == zoomReset` (the "⌘0 is not a slot" assertion removed).

#### Rust landing
- `crates/mx-core/src/model/tab.rs`: `hotkey: Option<String>` (chord id; mx-core has no chord type) — or reuse `mx-plugins/src/chord.rs` type if mx-core may depend on it (it may not; keep id string in core).
- `crates/mx-core/src/persist/pane.rs` (+ writer): read/write `"hotkey"`; exclude from recipe serialization (`group.rs` recipe).
- `crates/mx-store/src/tabs_ops.rs`: delete `select_index`; add `set_hotkey(id, Option<String>)` with steal + banners. Hook the "strip from panels" into whatever `prefs.rs::set_shortcuts`/bind path the app calls (store side, so banner text lives in store). Load-time drop of action-owned keys needs the keymap: do it in mx-app after restore (`keymap/settings.rs`), or pass an `owner` predicate into restore.
- `crates/mx-app/src/keymap/chord.rs`: delete `Chord::slot`, `slot_of`; recorder (`settings.rs` / `check_chord_for`) stops rejecting digits.
- `crates/mx-app/src/keymap/dispatch.rs`: replace `Hit::Slot(usize)` with `Hit::Panel(String)`; order action → panel → plugin in both window and terminal paths (currently `slot().or_else(plugin)` in one branch, `plugin.or_else(slot)` in the other — unify to panel before plugin).
- `crates/mx-app/src/keymap/tests.rs`, `tests_store.rs`: replace slot tests with the cases above.
- New dialog `crates/mx-app/src/chrome/dialog/hotkey.rs` (recorder reused from `chrome/settings/shortcuts.rs` — make the recorder a shared widget); menu item in `crates/mx-app/src/chrome/menu/builders.rs` (panel menu) + action in `chrome/menu/actions.rs` / `app/menu_actions.rs`.
- Display: `crates/mx-app/src/panes/header.rs`, `panes/view_header.rs` (key hint), `sidebar/tab_row.rs` (trailing label); remove the "Fixed" slot row in `chrome/settings/shortcuts.rs`.

#### Already in Rust
Full 2.4 slot system implemented (`Chord::slot`, `slot_of`, `Hit::Slot`, `select_index`, tests `slot_chords_are_intercepted_inside_the_terminal`, `slot_hit_selects_the_nth_session`, `recorder_refuses_unknown_keys_and_slots`, `linux_labels_ids_slot_and_paste`) — all must be removed/rewritten.

---

### TASK#82250 — Drag panes by the header (5c7af80)

#### Behaviour
- `PaneGrab` wraps the header of every pane kind (terminal `_PaneHeader`, `_DocHeader`, `_SetupHeader`, plugin `_Header`).
- Only when `store.paneCount >= 2`; with one pane the header is not draggable at all (no Draggable in tree).
- Drag payload: the tab (same payload as a sidebar row drag) → reuses existing pane drop (`_PaneSlot`: edge = split on that side, middle = swap; `AppStore.dropTab` already removes the tab from its old position).
- Drag starts only after pointer movement past the platform drag threshold (Flutter default `kTouchSlop`-ish for mouse; a stationary click stays with header buttons such as pin).
- Anchor: fixed `Offset(18, 17)` from the feedback's top-left, regardless of grab point; `DragCursor.grab = anchor` on start (so side computation uses pointer = corner + anchor).
- On drag start: `store.focusPane(tab)`.
- Cursor over header: `SystemMouseCursors.grab`.
- Original header while dragging: opacity 0.4.
- Feedback `_LiftedPane`: maxWidth 280, height 34, padding h12, bg `Mx.bgSidebar`, radius 10, border 1.4 px `tint ?? Mx.accent@0.55`, shadow `Mx.shadow` blur 18 offset (0,8); row: `Icons.open_with` 14 fgDim, gap 8, title ellipsis `paneTitleStyle` 13 `Mx.fg`.
- Over itself: drop-side hint is suppressed (no "Dividir"/"Trocar" label); dropping on itself is accepted and changes nothing (treated as a click).

#### config.json / shortcuts
None. Empty-screen hint text changed (see session cards).

#### Tests (pane_grab_test; 1800×900 viewport, three claude tabs side by side, grab at header topLeft + (60,20), move by (30,0) then to target)
- Given [um,dois,tres]; when dragging tres's header onto um at (x 0.05, y 0.5); then order [tres,um,dois] and focused pane = tres.
- Given [um,dois,tres]; when dragging um onto dois at (0.5, 0.95); then root has 2 children, order [dois,um,tres], first child is a column split.
- Given [um,dois]; when dragging um onto dois center (0.5,0.5); then pane order [um] and both tabs still exist (session not closed).
- Given [um,dois]; when dragging um by (40,200) over itself; then neither "Dividir" nor "Trocar" visible; after release order unchanged.
- Given [um,dois]; when tapping the pin button (`push_pin_outlined`) in um's header; then um is pinned and order unchanged.
- Given [um]; then no draggable in the tree.

#### Rust landing
- `crates/mx-app/src/panes/header.rs` (+ `view_header.rs` for doc/setup/plugin headers): gate `.on_drag` on pane count ≥ 2; on drag start focus pane; grab cursor; dim original 0.4.
- `crates/mx-app/src/app/drag.rs`: header ghost variant (34 px, max 280, open_with icon, tint border 1.4, anchor 18/17). Keep sidebar-row ghost as is.
- `crates/mx-app/src/panes/mod.rs::on_drag_move`: if `ev.drag.id == id` → hint None.

#### Already in Rust
`panes/header.rs` already has `.on_drag(DraggedTab…)` with `TabGhost` (240 wide, 14.5 semibold, radius 12, offset −20/−16) — discrepancies: draggable even with one pane, no focus-on-start, no self-hover suppression, ghost geometry/anchor differ, no grab cursor/dim. Header also still renders `±dirty` chip and subtitle — both removed in 610086e (see below).

---

### TASK#82250 — Session cards and plugin tiles on the empty screen (5c7af80)

#### Behaviour
`EmptyPane` (shown where no pane is open) is rewritten; old headline/cheat sheet removed. Layout: `SingleChildScrollView` padding 28 all sides; content width = maxWidth − 56.

`SessionCards.of(store)` = tabs where `!exited && !embedded && !isPluginView && (owner == null || plugins.byId(owner) == null)`.

Mode A — `of(store)` empty ("apps" screen), centered vertically (minHeight = maxHeight − 56):
- `'O que vamos abrir?'` (fg, 18, w600); gap 6; subtitle (fgFaint 12.5): no rail plugins → `'Instale um plugin, ou abra uma sessão pela lateral'`, else `'Escolha um plugin, ou abra uma sessão pela lateral'`; gap 32; `PluginTiles(large: true)`.

Mode B — sessions exist: `PluginTiles` (small strip) → gap 30 → `SessionCards` → hint text (centered, fgFaint 11.5, height 1.45): `'Arraste um cartão ou uma sessão da lateral pra cá — pela borda de um painel, ela divide a tela; pelo meio, toma o lugar dele'`.

**SessionCards** — sections in this order, each only if non-empty; classification first-match:
1. `'Esperando você'` dot `Mx.yellow`: claude && !done && status.needsHuman.
2. `'Terminaram enquanto você não via'` dot `Mx.green`: unseen && rested && !done.
3. `'Trabalhando'` dot `Mx.accent`: claude && status ∈ {working, tool, starting}.
4. `'Em repouso'` dot `Mx.fgFaint`: rest.
Section header: dot 6×6 + gap 8 + label (fgDim 12 w600) + gap 6 + count (fgFaint 11.5 mono). Gap 8 → `Wrap(spacing 12, runSpacing 12)` → gap 22.
Columns: `cols = clamp(floor((width+12)/(280+12)), 1, 99)`; card width = `(width − 12·(cols−1))/cols`.

Card (`_SessionCard`), height 118, radius `Mx.radius`, clip antiAlias, animated 140 ms easeOut:
- bg `bgSidebar` (hover `bgHover`), hover translateY −2 and shadow blur 16 offset (0,6); border: hover `accent@0.6`, else attention (claude && !done && needsHuman) `status.color@0.45`, else `Mx.border`.
- Tint (tab/group tint) → 3 px left stripe full height.
- Padding 14,12,12,10. Row: glyph box 30 (`TabGlyph`, same as the sidebar leading — extracted from sidebar), gap 10, column [title (13.5 w600; fgDim if done else fg; 1 line ellipsis), `where` (fgFaint 10.5 mono)], trailing hotkey chip if set (padding h5 v1.5, bgActive, radius 4, mono 10 fgDim).
  - `where` = join(' · ') of folder.name (if not loose and title != folder.name) and branch (if non-empty and != '(detached)').
- Gap 9, body (Expanded, max 2 lines, fgDim, height 1.4): plain shell (no launcher) → cwd with `$HOME` → `~` (mono 11.5); claude whose subtitle == status.label or == 'Pronto' → `hooks.lastPrompt`; else subtitle (12). Null → `'Nada dito ainda'` (fgFaint 12 italic).
- Footer row: dot 6 state color + gap 6 + state label (11, same color); spacer; done → `Icons.task_alt` 13 green; followUps → `FollowUpMark`; `restedAgo` (11, green if unseen else fgFaint).
  - State: claude → (status.label, status.color); shell → `'Programa'` if launcher else `'Terminal'`; reader `'Documento'`; setup `'Configuração'`; plugin `'Plugin'` — all fgFaint.
- Click: if tab in a group → `openGroup(group)`; else `select(tab)`. Right-click → `showPanelMenu`. Cursor click. Draggable (payload tab, feedback = card at 0.92 opacity, original 0.3) — dropping onto the empty area is the existing `_EmptyDrop`.

**PluginTiles** (`store.railPlugins`, same set/order as the rail):
- Small strip: header `'Plugins'` (no dot) + count; `Wrap(spacing 10)`; cols from tileMin 220, gap 10. Tile 58 high, padding h12, radius `Mx.radius`, bg bgSidebar/hover bgHover, border hover `accent@0.6` / shown `accent@0.35` / `Mx.border`; 32×32 glyph box (bgActive, radius 8, glyph 18 fg if lit else fgDim), gap 10, name (13 w600) + description (11 fgFaint, 1 line), badge.
- Large (apps screen): fixed tile width 196 (clamped to width), centered Wrap spacing 16; tile 184 high, radius 16, hover translateY −3 + shadow blur 20 offset (0,8); padding 16,22,16,14; 64×64 glyph box radius 16 (lit `accent@0.16` else bgActive, glyph 30); gap 14; name 14.5 w600 centered; gap 4; description 11.5 fgFaint height 1.35 max 2 lines; badge at top 12 right 12.
- Badge: plugin busy (`plugins.busy` has `"<id>/…"`) → 8×8 accent dot; else `sidebarBadges[id]` → pill (h6 v2, bgActive, radius 8, 10.5 w600 fg); open-window count is not shown.
- Click: if sidebar not hidden and shownPlugin == this → no-op; else `showSidebarView(id)` (never toggles the sidebar off, unlike the rail icon).
- Trailing "+" tile `_AddPluginTile`: opens `showPluginCatalog`. Small: `Icons.add` 16 + `'Instalar plugin'` (12). Large: 64×64 bordered box with add 26, `'Instalar plugin'` (13), `'Do catálogo'` (11.5 fgFaint). Transparent bg, hover bgHover, border `Mx.border`; color fgFaint / hover fgDim.

#### config.json / shortcuts
None.

#### Tests (session_cards_test; 1200×900)
- Given no tabs; then `'O que vamos abrir?'` and `'Instalar plugin'` shown, `'Plugins'` and `'Esperando você'` absent.
- Given claude "pede permissão" (waitingPermission, activeTool Bash), claude "terminou" (idle, restedAt now, unseen), shell "um shell"; then all three titles, `'Terminaram enquanto você não via'`, `'Em repouso'`, `'Esperando você'`, and body `'Quer aprovação pra Bash'` shown; vertical order pede permissão < terminou < um shell.
- Given shell "um shell"; when tapping its card; then `store.isOpen(tab)`.
- Given plugin `git` (name Git, description "O status do repo", icon terminal, one command) and no tabs; then `'O que vamos abrir?'`, `'O status do repo'`, `'Instalar plugin'`; tap "Git" → `shownPlugin.id == 'git'`; tap again → `sidebarHidden == false`; add shell "um shell" + notify → `'O que vamos abrir?'` gone, `'Plugins'`, `'O status do repo'`, `'um shell'` shown.

#### Rust landing
- New `crates/mx-app/src/panes/session_cards.rs` (cards + sections + column math, pure fns for classification/body/where/state testable without gpui).
- New `crates/mx-app/src/panes/plugin_tiles.rs`.
- Rewrite `crates/mx-app/src/panes/empty.rs` (`empty_pane`).
- Extract `TabGlyph` from `crates/mx-app/src/sidebar/tab_row.rs` into `crates/mx-app/src/sidebar/glyph.rs` (shared).
- Classification helper can live in `crates/mx-store/src/sidebar.rs` (or a new `mx-store/src/empty_screen.rs`) if it needs store state (railPlugins, groups).

#### Already in Rust
`panes/empty.rs` implements the 2.4 screen (`headline()` "Nenhum painel aberto"/"N sessões rodando…", `DRAG_HINT` old text, test `the_headline_counts_sessions_left_running`) — replace; `empty_screen(plugin)` (plugin `screen` empty state) is unrelated, keep.

---

### TASK#82252 — Attach background sessions from the history (c707872)

#### Behaviour
- `AgentInfo` gains `id: String?` read from `claude agents --json` key `"id"` (job id; only `kind == "background"` rows have it).
- `ChatStanding` gains `background` (between `live` and `gone`), note `'em segundo plano · anexar'`; `opens` is true for `fresh` and `background`. `live` doc narrowed to "interactive in another terminal".
- History row tint: `background → Mx.accent` (onScreen green; live/gone yellow; fresh fgFaint).
- `standingOf(chat)`:
  1. `_tabHolding(sessionId)` != null → `onScreen`. `_tabHolding` = first tab with `(kind == claude || !exited)` and (`resumeId == sid` || `hooks.sessionId == sid`). (Shell tabs count only while alive.)
  2. `_elsewhere(sid)`: first `agents.latest` row with `sessionId == sid` and `pid ∉ {pid of every tab's term}` → `background` if `kind == 'background' && id != null`, else `live`.
  3. missing / empty cwd / cwd not a dir → `gone`; else `fresh`.
- `resumeChat`: `onScreen` → select `_tabHolding`; `background` → `_attach(chat, agent)`:
  - folder = given folder ?? `folderAt(chat.cwd)` ?? loose; cwd = chat.cwd if it exists else folder.root.
  - `openShell(folder, cwd: cwd, command: "claude attach <shell-quoted id>", featureOrHotfix: …)` — a **shell** tab, not a claude tab (not persisted for `--resume`).
  - `tab.customLabel = chat.label`, `tab.agentName = agent.name`, `tab.sessionId = chat.sessionId`.
- `applyAgents`: a tab whose `hooks.sessionId != null` no longer has its `sessionId` overwritten by the agents poll (CLI registry keeps the birth id across `/clear`; avoids 30 s flip-flop).
- Test hook: `TermSession.fakePid` (test only).

#### config.json / shortcuts
None. External command: `claude attach '<jobId>'` (run inside the login shell via the shell-tab `command`).

#### Tests (history_test)
- Given folder /tmp, chat `titled`, agents = [{pid null, id '40e816ba', cwd /tmp, kind background, sessionId titled, name 'revisão do schema'}]; then standing = background and note contains 'segundo plano'; when resumeChat → exactly one openShell request `{folder:/tmp, cwd:/tmp, command:"claude attach '40e816ba'"}`, tab kind shell, title = chat.label, agentName 'revisão do schema'; then standing = onScreen and second resumeChat returns the same tab without a new request; when the shell exits, standing = background again.
- Given a background agent row without `id`; then standing = live.
- Given a tab resumed on chat `old` with pid 4242, hooks `SessionStart{session_id: now}`, agents = [{pid 4242, interactive, sessionId old}]; then standing(now) = onScreen and resumeChat(now) returns that tab; after `tab.sessionId = now`, standing(old) = fresh.
- Given claude tab pid 4242, agent {pid 4242, interactive, sessionId 'velho'}; when applyAgents → resumeId 'velho'; when hooks then report a new session id and applyAgents runs again → resumeId stays the hooks id.
- Existing: "live outside maestria warns instead of opening" unchanged.

#### Rust landing
- `crates/mx-core/src/model/agent.rs`: `id: Option<String>` (`"id"`).
- `crates/mx-sys/src/history.rs`: `ChatStanding::Background`, note, `opens`.
- `crates/mx-store/src/history_ops.rs`: `tab_holding`, `elsewhere`, new standing logic, `attach()` via existing shell-open path in `tabs_open.rs` (needs a `command` param — check `open_shell` signature).
- `crates/mx-store/src/ingest.rs::apply_agents`: skip session_id overwrite when `hooks.session_id.is_some()`.
- `crates/mx-app/src/app/chats.rs`: accent tint for background rows.

#### Already in Rust
`history_ops.rs` implements the 2.4 logic (resume_id-only onScreen; any agent row = Live). `AgentInfo` lacks `id`. `ChatStanding` lacks Background.

---

### TASK#82249 — Compact switch + settings visual tweaks (610086e)

#### Behaviour
- New `MxSwitch` replaces every Material `Switch`/`SwitchListTile`: track 30×18, radius 9, inner padding 1 (inset 2), knob 14 circle.
  - On: track `Mx.accent` (hover: lerp(accent, white, 0.08)); border same color as track; knob = `Mx.canvas` if accent luminance > 0.5 else white, shadow `Mx.shadow` blur 2 offset (0,1); aligned right.
  - Off: track `Mx.bgActive` (hover `Mx.bgHover`); border `Mx.border` (border present in both states so the knob does not jump); knob `Mx.fgDim`, left.
  - Focus ring: shadow `accent@0.5` spread 2. Animations 140 ms easeOut. Disabled (`onChanged == null`): opacity 0.45, basic cursor, no response. Keyboard activate toggles. Semantics toggled/enabled.
- Settings `_Toggle` row (replaces `SwitchListTile`): whole row clickable (opaque), vertical padding 6; title (12.5 fg) + optional subtitle (gap 2, 11.5 fgFaint height 1.4); gap 24; switch. Switch aligned to the title (cross-axis start) when there is a subtitle, centered otherwise. Applied to: "Barra de status em cima", "Hibernar sozinho o que está parado fora da tela", "Atualizar sozinho", "Atualizar os plugins sozinho" (**its subtitle was removed**), "Enviar sozinho ao terminar de falar". Strings unchanged otherwise.
- Settings plugin row: state shown only as icon color with `Tooltip(state.label)` on the glyph; enable switch = `MxSwitch` (was FittedBox Switch 24 high).
- Plugin settings boolean (`_SettingRow`) and suggestion card `_PluginSwitch` use `MxSwitch`.
- **Removed `dirty` count**: `MxTab.dirty` field and the git `dirtyCount` refresh are gone; sidebar row yellow 6 px dot and pane-header `_Chip('<n>')` removed.
- Pane header: subtitle text removed from the terminal pane header (title + chips + buttons only).
- Rail: open-window count badge removed from plugin rail icons (only busy dot or plugin badge; count stays in tooltip).
- `ResultChip` (pane header "N arquivos" chip) restyled: outlined pill, padding L6 R3 T2 B2, radius 5; closed: transparent (hover bgHover), border `Mx.border`, ink fgDim (hover fg); open: bg `accent@0.12`, border `accent@0.5`, ink accent. Icon `Icons.edit_note_rounded` 14 (was `inventory_2_outlined` 11, green filled), text `'1 arquivo'`/`'<n> arquivos'` (11), trailing chevron `expand_less_rounded`/`expand_more_rounded` 14. Tooltip: `'Esconder os arquivos que esta sessão escreveu'` / `'Ver os arquivos que esta sessão escreveu'` (was 'Esconder o que mudou'/'Ver o que mudou').
- Removed leftover debug file write in pty wheel handler (no behaviour).

#### config.json
None. (`dirty` was never persisted.)

#### Tests
- switch_test: tap on `MxSwitch(value false)` → callback receives true; `onChanged null` tap → no exception; size == 30×18.
- suggestion_plugins_test: switch finder now `MxSwitch` (absent/present cases unchanged).

#### Rust landing
- `crates/mx-app/src/chrome/widgets/controls.rs`: split current `Switch` into a bare `MxSwitch` (30×18) + `Toggle` row; update callers in `chrome/settings/{appearance,plugins,programs}.rs`, `plugin_ui/settings.rs`, suggestion card (wherever `_PluginSwitch` lives in mx-app).
- `crates/mx-core/src/model/tab.rs`: drop `dirty`; `crates/mx-store/src/git_ops.rs`: drop dirty_count polling; `crates/mx-sys/src/git.rs` `dirty_count` may become dead.
- `crates/mx-app/src/panes/header.rs`: drop `±dirty` chip and subtitle; restyle result chip.
- `crates/mx-app/src/sidebar/tab_row.rs`: drop dirty dot; `crates/mx-app/src/rail/*`: drop open-count badge.

#### Already in Rust
`controls.rs::Switch` is a combined title/subtitle/switch row (2.4 style: knob `canvas` when on, regardless of accent luminance; no hover/focus/disabled state). `dirty` still modelled and rendered (`header.rs` `±{dirty}` — note Rust used `±n`, Dart had plain `n`).

---

### TASK#82254 — Plugin `folders` setting in a picker modal; setting descriptions as tooltip (281c232)

Depends on area F (b043ae0 introduces `PluginSettingType.folders`: value = list of folder `root`s; default `[]`; options = sidebar folders). This commit only changes its UI and all settings' description rendering.

#### Behaviour
- All plugin setting rows (`_SettingRow`): description no longer rendered below the title; instead, if non-empty, a `Icons.help_outline` 14 fgFaint icon 6 px after the title with `Tooltip(description, maxWidth 360)` and help cursor. Title is `Flexible`.
- `folders` row: one line `[label (expanded)] gap 12 [OutlinedButton width 200] [reset]`, bottom padding 10.
  - Button: padding h10 v8, min height 32, left aligned, border `Mx.border`, radius 6; content `Icons.folder_outlined` 14 fgDim, gap 6, summary (12.5; fgFaint if nothing shown else fg, ellipsis), `Icons.unfold_more` 14 fgFaint.
  - Summary over picked roots that are still in the sidebar (names): 0 → `'Nenhuma'`; 1–2 → names joined `', '`; >2 → `'<a>, <b> e mais <n-2>'`. No sidebar folders → button disabled, text `'Nenhuma pasta na lateral'`.
  - On "Pronto": if result equals `setting.resolve(null)` (default) → store null (key removed from config), else the list.
- `_FolderPicker` dialog (bg bgSidebar, width 520):
  - Header (padding 16,14,10,4): setting title (14 w600 fg); `'<count> de <total>'` (11.5 fgFaint; count = sidebar folders picked); TextButton `'Todas'`/`'Nenhuma'` (12) acting **only on currently filtered rows** (`'Nenhuma'` when all shown are picked; disabled when none shown).
  - Filter field (padding 12,4,12,8), autofocus, hint `'Filtrar pastas'`, search icon 16; whitespace-split terms, all must be contained (case-insensitive) in `"<name> <root>"`.
  - List maxHeight 380; empty → `'Nenhuma pasta com esse nome'` (12 fgFaint). Row: compact Checkbox + name (12.5 fg) over root (10.5 mono fgFaint, ellipsis); whole row toggles.
  - Footer: `'Cancelar'` (returns null → no change), FilledButton `'Pronto'` returns picked roots in **sidebar order**, followed by picked roots not in the sidebar (kept, not dropped).

#### config.json
Plugin settings object (spec 04 §1.6) — `folders` value list of roots; default (empty) is omitted.

#### Tests (plugins_test "folders mostra as marcadas num botão, que abre o modal das pastas"; sidebar folders app=/r/app, api=/r/api, in that order)
- Given the settings dialog; then no Checkbox inline.
- When opening via `'Nenhuma'`, tapping api, Pronto → value `['/r/api']`.
- When opening via `'api'`, tapping app, Cancelar → still `['/r/api']`.
- When opening via `'api'`, tapping Todas, Pronto → `['/r/app','/r/api']` (sidebar order) and button shows `'app, api'`.
- When opening, filtering `'api'` → one Checkbox; Nenhuma → `'1 de 2'`; clearing filter, Todas → `'2 de 2'`; Nenhuma; Pronto → `[]`, plugin settings JSON empty (default omitted), no `'Voltar ao padrão'` tooltip.

#### Rust landing
- `crates/mx-plugins/src/manifest/settings.rs`: `SettingType::Folders` (owned by area F if F lands the type; otherwise here).
- `crates/mx-app/src/plugin_ui/settings.rs`: description-as-tooltip for all rows; folders button + summary.
- New `crates/mx-app/src/plugin_ui/folder_picker.rs` (modal; pure fns `summary(names)`, `filter(query, folders)`, `ordered_result(folders, picked)` unit-testable).

#### Already in Rust
No `folders` setting type at all (`SettingType` = String/Number/Boolean/Color/Select); plugin settings render description under the title (2.4).


## F — Sessions API, notifications, plugin settings extras (b043ae0)

### TASK#82254 (b043ae0) — Sessions API for plugins, notifications with a portrait

Source: `lib/services/{plugin_api,plugins,notify,hooks,pty,store,plugin_catalog}.dart`, `lib/ui/{plugin_dialogs,settings}.dart`,
`macos/Runner/MainFlutterWindow.swift`, `docs/plugins.md`, tests `hibernate_test`, `plugins_test`, `plugin_input_test`,
`hook_reducer_test`, `plugin_catalog_test` (+ `flow/history/menus_test` only add `bool place = true` to fake `openClaude` overrides).
Baseline: 04-plugins §1.8, §2.3–2.6; 03 §1.3, §4.

#### F.1 Behaviour summary

A plugin (the "escritório"/office plugin is the motivating client) can now drive Claude sessions end to end:
rename, wake, hibernate, close a user Claude session, open one in background, type real keys (answer
`AskUserQuestion`), paste a screenshot into the prompt, set a per-session PNG "portrait" used by the OS
notification, and observe rename / context / sleep / modal-close events. The host refuses to type into a Claude
terminal that is showing the folder-trust prompt. Two hook events (`PreCompact`, `PostCompact`) are now subscribed
so plugins with `hooks` see compaction. New plugin setting type `folders`. Manifest-derived plugin "kind" tag
(`Tema` / `Recurso`) in the installed list and catalog (catalog half overlaps area B — the `kinds` catalog field).

#### F.2 Permissions (04 §1.8 replaced)

Enum order (affects the "permissão desconhecida" message list): `hooks`, `terminal.write`, `sessions.create`,
**`sessions.close`**, `notifications`.

| permission | label (verbatim, install dialog) | gates |
|---|---|---|
| `hooks` | unchanged | unchanged (+ now receives `PreCompact`/`PostCompact` hook events) |
| `terminal.write` | unchanged | `terminal.sendText`, **`terminal.sendKeys`**, **`terminal.pasteClipboard`** |
| `sessions.create` | unchanged | `session.openClaude`, `session.openShell`, **`session.wake`**, **`session.rename`** |
| `sessions.close` (new) | `encerrar sessões do Claude quando você pedir` | **`session.close` of a user Claude session**, **`session.hibernate`** |
| `notifications` | unchanged | `window.notify` |

Unknown-permission manifest error now lists: `... as que existem são hooks, terminal.write, sessions.create, sessions.close, notifications`.
`session.portrait` needs **no** permission.

#### F.3 `initialize` capability flags (added by this commit; all value `1`)

`openInBackground`, `pasteImage`, `keys`, `wake`, `hibernate`, `portrait`, `closeSessions`, `rename`.
Full 2.7 key order of `initialize` params (for wire-identical output; flags not listed above belong to area D/TASK#82253):
`apiVersion, blocks, rfw(2), floats, input, form, modal, editor, sprites, zoom, textInput, openInBackground, markdown,
pasteImage, keys, arrows, wake, hibernate, portrait, closeSessions, modalFrom, selectable, rename, pluginId, pluginDir,
dataDir, permissions, settings`. (Note: v2.4 HEAD's `initialize` had no `editor` key — 04 §2.3 already lists it; harmless.)

#### F.4 Protocol — new / changed plugin → host methods

Common: `_tab(p)` resolves `tabId` (unknown → -32602 `não há painel "<id>"`). Where noted "tabId required", a
missing/non-string `tabId` → -32602 `"tabId" é obrigatório` **before** any lookup (no focused-pane fallback).
**Trust gate** (`_trustGate`): if target `kind == claude` and the terminal's visible screen matches
`/trust this folder|Do you trust the files/i` → -32602
`"<title>" está perguntando se você confia na pasta: responda no terminal dele`.

| method | params | result | perm | rules / errors |
|---|---|---|---|---|
| `terminal.sendText` (changed) | `text`*, `submit?`, `tabId?` | `null` | terminal.write | Target resolution unchanged. After the passive/exited/hibernated check, **trust gate**. `submit:true` with `text.trim()==""` → send a lone Enter (`\r`, "send what is already in the prompt", e.g. a pasted image) instead of type+Enter. `text` still required string (`""` allowed). |
| `terminal.sendKeys` (new) | `keys`*: list, `tabId?` | `null` | terminal.write | Target via `_sessionTab` (same resolution as sendText). Passive/exited/hibernated → -32602 `"<title>" não tem processo pra receber teclas`. Trust gate. Validation: `keys` is a non-empty list, length ≤ 200, every item is one of `up down left right enter escape tab space backspace` or an object `{text: string}`; else -32602 ``\`keys\`: uma lista de up, down, left, right, enter, escape, tab, space, backspace ou { text }``. Delivery: one item at a time, **60 ms gap** between items (none before the first); stop silently if the session exits mid-way; `{text}` → terminal text input (typed, not bracketed paste); named key → the emulator's key encoding (honours DECCKM: `up` = `ESC[A` or `ESC OA` in application-cursor mode; `enter`=`\r`, `escape`=`\x1b`, `tab`=`\t`, `space`=` `, `backspace`=`\x7f`). Awaited: the RPC returns after the last key. |
| `terminal.pasteClipboard` (new) | `tabId?` | `null` | terminal.write | Passive/exited/hibernated → -32602 `"<title>" não tem processo pra receber a colagem`. Trust gate. Behaves as ⌘V in the terminal: clipboard text → bracketed paste; no text (an image) and tab is claude → write `\x16` (^V, Claude Code reads the image from the clipboard itself). Non-claude + image → nothing. |
| `session.openClaude` (changed) | `cwd?`, `prompt?`, `label?`, **`background?`** | `{tabId}` | sessions.create | `background:true` → store `openClaude(..., place:false)`: tab registered (sidebar, `session.opened` emitted) but **not placed in a pane and no screen switch**. Default places as before. |
| `session.rename` (new) | `tabId`* (required), `title`* | `{title}` | sessions.create | `title` trimmed, truncated to 80 chars; applied via the same path as the sidebar rename (sidebar, header, history). Empty → back to the default name (folder/branch). Result = the resulting display title (`tab.title`). Emits `session.renamed`. |
| `session.wake` (new) | `tabId`* (required) | `{woke: bool}` | sessions.create | Not hibernated → `{woke:false}` (no-op). Else awaits `store.wake(tab)` → `{woke:true}`; status becomes `starting` immediately (event `session.status` with `hibernated:false`), later `ready` (event via settle, §F.6). |
| `session.hibernate` (new) | `tabId`* (required) | `{hibernated: bool}` | **sessions.close** | Already hibernated → `{hibernated:false}`. `!canHibernate(tab)` → -32602: if `kind != claude || exited` `"<title>" não é uma sessão do claude rodando`, else `"<title>" ainda não tem conversa pra retomar`. If `(status != ready && !status.atRest) || hooks.busyForks` (atRest = idle\|waitingInput) → -32602 `"<title>" está trabalhando: hiberne quando ele parar`. Else `store.hibernate(tab)` → `{hibernated:true}`; event `session.status` `hibernated:true`. |
| `session.portrait` (new) | `tabId`* (required), `image`: string\|null | `null` | none | `kind != claude` → -32602 `só uma sessão do claude tem retrato`. `image: null` (or key absent) → clear portrait. Else `image` must be a String ending `.png` (case-insensitive) that exists → else -32602 `"image": o caminho de um .png que existe`. Resolve to absolute real path (symlinks resolved); must start with `realpath(pluginDir) + "/"` or `realpath(dataDir) + "/"` (a dir that doesn't exist never matches) → else **-32001** `"image" tem que estar na pasta do plugin ou na de dados dele`. Stores the real path on the tab (`portrait`, in-memory only, never persisted — plugin re-sends on startup). |
| `session.close` (changed) | `tabId`* (**now required**) | `null` | — / sessions.close | Missing → -32602 `"tabId" é obrigatório` (breaking vs 2.4, which fell back to the focused tab). Allowed if `tab.owner == caller` (as before) OR (`kind == claude && owner == null` AND caller has `sessions.close`). Else -32001: for a user claude tab `fechar uma sessão do claude pede a permissão "sessions.close"`, otherwise `só dá pra fechar um terminal que o plugin abriu com "owned"`. Closing = `store.closeTab` (conversation stays in history). `sessions.close` does NOT allow closing user shell tabs. |
| `view.open` | (`from?`, `width?`, `height?`) | | | belongs to area D (TASK#82253), not this item. |

#### F.5 Session JSON (04 §2.5) — new field

`"context": {"percent": int, "tokens": int, "window": int} | null` inserted **before `owner`** (key order:
`... launcher, context, owner, embedded, tag?, activity?`). `null` before the first status-line report and right after
a `/compact`. Source = the tab's `ContextUsage` (03 §3.1).

#### F.6 Events (04 §2.6) — new / changed

| type | fields | activation | when |
|---|---|---|---|
| `session.status` (changed) | `tabId`, `status`, `previous`, **`hibernated?`** | `onSession` | Also emitted (a) on every hibernate (auto, sidebar, plugin): `{status:"ended", previous:<before>, hibernated:true}`; (b) on every wake: `{status:"starting", previous:<before>, hibernated:false}`; (c) when the launch/wake settle turns `starting→ready`: `{status:"ready", previous:"starting"}` (no `hibernated`). Previously (c) never reached plugins (they saw "starting" forever). |
| `session.renamed` (new) | `tabId`, `title` | `onSession` | Every tab rename (sidebar or plugin), only for non-passive tabs; emitted before save. |
| `session.context` (new) | `tabId`, `context` (same shape as F.5, or `null`) | `onSession` | Whenever the stored context usage actually changes (the existing "all three equal → return" dedupe in 03 §3.1 suppresses it). |
| `view.closed` (new) | `viewId` | none — direct delivery to the **owner only**, only if the plugin is active and its connection alive (does not start it) | When the plugin's modal is dismissed by the user or closed via `view.close` (`closeModal`). `view.update` still returns `open:false` afterwards (compat). |
| `hook` | — | `onHook:PreCompact`, `onHook:PostCompact` | New hook names arrive (needs `hooks`). |

#### F.7 Hooks (03 §1.3/§1.4)

`settingsFor` JSON gains, after `SubagentStop`, `"PreCompact": [group()]`, `"PostCompact": [group()]` (no matcher,
same `{type:"http", url, timeout:5}` entry). Key order: SessionStart, SessionEnd, UserPromptSubmit, PreToolUse,
PostToolUse, PostToolUseFailure, Notification, Stop, StopFailure, SubagentStop, PreCompact, PostCompact.
`HookReducer` has no case for them → status untouched (a manual `/compact` must not produce a "finished" transition,
nor trigger follow-up queue/alerts). They still flow to plugins as `hook` events.

#### F.8 Launch settle with trust prompt (03 §1.3 "3 s after launch")

Old: one timer at 3 s → `hooks.settle()`. New: `settleSoon(3s, tries=300)`: when it fires, if tab gone/exited → stop;
if the terminal `asksTrust` → reschedule after **2 s** with `tries-1` (stop when tries hits 0; ≈10 min max); else if
`hooks.settle()` changed status → emit `session.status {ready, previous: starting}` (activation `onSession`) and notify.
Applies to fresh launches and wakes (wake relaunch goes through the same launch path).

`asksTrust` = regex `trust this folder|Do you trust the files` (case-insensitive) over `screenText()`: the visible
viewport (last `viewHeight` buffer lines), each line's first `min(viewWidth, len)` cells, code point 0 → space, `\n`
after each line.

#### F.9 Notification with portrait (03 §4 `checkAlerts` / `Notifier.alert`)

`checkAlerts`: for each newly-waiting tab (`!done && needsHuman`, key `tab:<id>` not yet alerted):
- tab has `portrait` → `alert(title: tab.title, body: alertBody(tab), subtitle: "MaestrIA", image: portrait, tabId: tab.id)`;
  set the click handler once (`onTap ??= focusFromNotification` → `select(tab)` if it still exists).
- otherwise unchanged: `alert("MaestrIA", "<title> · <callToAction>")`.

`alertBody(t)`: `waitingAnswer` with `hooks.question != null` → `pergunta: <question>`; `waitingPermission` with
`hooks.activeTool != null` → `quer aprovação pra <activeTool>`; else `status.callToAction`.

`alert(title, body, {image, subtitle, tabId})`:
- **Linux**: `notify-send -a maestria [-i '<image>'] '<title>' '<body>'` (via `Sh.run`, shell-quoted). `subtitle` and
  `tabId` ignored, no click handling. (Note: the Dart 2.4/2.7 string has **no** `--`; the Rust `alert_command` adds
  `-- ` — keep `-i` before `--`: `notify-send -a maestria -i <img> -- <title> <body>`.)
- **macOS with image**: native `UNUserNotificationCenter` via method channel `notify` `{title, body, subtitle?, image, tabId?}`:
  `requestAuthorization([.alert,.sound])` (system prompt first time); denied → returns false; else content
  title/body/subtitle (if non-empty), `userInfo.tabId`, attachment = **copy** of the PNG to
  `$TMPDIR/<uuid>.png` (identifier `portrait`; attaching moves the file), request id = UUID, no trigger. Delegate
  `willPresent` → `[.banner, .list]` (show while app is frontmost); `didReceive` → `NSApp.activate`, send
  `notificationTapped(tabId)` → `onTap(tabId)`. If the native call returns false / is missing / throws → fall back to
  the plain `osascript display notification "<body>" with title "<title>"` (title = session title; subtitle lost).
- macOS without image: unchanged osascript.

No size constraint on the PNG is enforced by the host (plugin's own file; the office plugin ships sprite PNGs).

#### F.10 Plugin setting type `folders` (04 §1.1 `contributes.settings[]`)

- `type: "folders"`: value = list of folder `root` strings (same `root` as `folders.list`). `fits` = list whose items are all
  strings; resolve returns a typed `List<String>` copy; fallback default `[]`. Non-fitting saved values (`['/a', 3]`,
  `'/a'`) → `[]`.
- Settings dialog row (rendered below its label, not beside): header row = label, then a text button `Todas` (when not all
  sidebar roots are picked) / `Nenhuma` (when all are), shown only if the sidebar has folders, then the reset button.
  Empty sidebar → faint text `Nenhuma pasta na lateral`. One row per sidebar folder (sidebar order): checkbox (compact),
  folder name (12.5), root in mono faint (11, ellipsis); tapping the row or checkbox toggles.
  Toggle appends to the end / removes. `Todas` = `picked + (roots not picked, in sidebar order)`; `Nenhuma` = picked minus
  current roots (roots of folders no longer in the sidebar are **kept**). If the result equals the default → `onChanged(null)`
  (removed from config, reset tooltip `Voltar ao padrão` hidden). "changed" uses deep equality.
- docs: `| folders | uma caixinha pra cada pasta da lateral, com "todas"/"nenhuma" | a lista dos root marcados ... |`.

#### F.11 Plugin kind tag (overlaps area B for the catalog side)

`PluginKind { theme('theme','Tema'), feature('feature','Recurso') }`. `manifest.kinds` = `{theme}` if `themes` non-empty,
plus `{feature}` if `hasProcess || commands || settings || sidebar || screen || suggestions != null`; can be both or empty.
Tag widget: padding h6/v1.5, radius 4, bg = color @12%, icon 11 (`palette_outlined` theme / `extension_outlined` feature)
+ label 10.5; color theme = accent, feature = fgDim. Installed list (settings `_PluginRow`) **replaces the state label**
with the tags (order theme, feature), before `Desenvolvimento`. Catalog entry: `kinds` array parsed, unknown ids dropped,
missing → empty (no tag); tags shown before `De fora`.

#### F.12 Tests (given / when / then)

hibernate_test:
1. Given a hibernated claude session; plugin `q` without perms → `session.wake {tabId}` errors; plugin with
   `sessions.create` and `{}` → errors; tab still hibernated. When `session.wake {tabId}` → `{woke:true}`, not hibernated,
   status `starting`. Again → `{woke:false}`.
2. Given a running claude session; plugin with only `sessions.create` → `session.hibernate` errors; with `sessions.close`
   and `{}` → errors. After hook `UserPromptSubmit` → hibernate errors, not hibernated. After `Stop` → `{hibernated:true}`,
   hibernated. Again → `{hibernated:false}`.
3. Given the terminal shows `Accessing workspace:\r\n\r\nQuick safety check: Is this a project you created or one you
   trust?\r\n❯ No, exit\r\n  Yes, I trust this folder\r\n` → `asksTrust` true; `terminal.sendText` and
   `terminal.sendKeys` (`text:"oi", submit:true, keys:["enter"]`) both fail with a message containing `confia na pasta`.
   After writing `\x1b[2J\x1b[H> ` → `asksTrust` false.
4. Portrait: plugin dir = temp dir with `rosto.png`; set → `tab.portrait == realpath(rosto.png)`; a png in another temp dir
   → error; non-existent `nada.png` in plugin dir → error; `image:null` → portrait null.
5. Rename: plugin without perm → error; with `sessions.create` but no tabId → error; title `'  login do google  '` →
   `{title:'login do google'}` and `tab.title` equals it; title `''` → returned title ≠ `'login do google'`.

plugin_input_test:
6. Given a claude tab with output capture and empty clipboard (image case: no text): `terminal.pasteClipboard` → written
   `['\x16']`; `terminal.sendText {text:'', submit:true}` → last write `'\r'`; plugin without `terminal.write` → error.
7. `openClaude(folder, start:false, place:false)` → tab in `tabs` but `isOpen` false; default `place` → `isOpen` true.

plugins_test:
8. kinds: themes only → `{theme}`; `main` only → `{feature}`; themes + commands → `{theme, feature}`; `{}` → empty.
9. `session.close`: store with claude `c1` (no owner) and shell `s1`; plugin `escritorio` with `sessions.close`, plugin `q`
   without. `q` closes `c1` → error; `escritorio` closes `s1` → error (permission is for claude sessions, not user
   terminals); `escritorio` with `{}` → error; `c1` still there. `escritorio` closes `c1` → removed, `s1` stays.
10. `folders` setting parse/resolve: type folders; `resolve(null) == []`; `resolve(<dynamic>['/a','/b'])` is a
    `List<String>` equal `['/a','/b']`; `resolve(['/a',3]) == []`; `resolve('/a') == []`.
11. Widget: plugin with setting `{id:'salas', type:'folders', title:'as salas'}`, sidebar folders `app (/r/app)`,
    `api (/r/api)`. Open settings → 2 checkboxes. Tap `api` → `['/r/api']`. Tap `Todas` → `['/r/api','/r/app']`.
    Tap `Nenhuma` → `[]`, plugin settings JSON empty (default removed), no `Voltar ao padrão` tooltip.
12. Session JSON: `context` null initially; after `tab.context = (71, 142000, 200000)` → `{percent:71, tokens:142000, window:200000}`.

hook_reducer_test:
13. After `Stop` (status idle), `PreCompact {trigger:manual}` → idle; `PostCompact {trigger:manual}` → idle.

plugin_catalog_test:
14. Entry `kinds:['theme','nova']` → `{theme}` (unknown dropped); entry without field → empty.

#### F.13 Rust landing

| piece | crate / file |
|---|---|
| `Permission::SessionsClose` + label, ALL order | `crates/mx-plugins/src/permission.rs` (+ update `tests/manifest.rs` expected message) |
| capability flags in `initialize` | `crates/mx-plugins/src/registry/lifecycle.rs` `initialize_params` (shared with area D — one owner should add all 2.7 keys in order) |
| method parsing/validation (`sendKeys`, `pasteClipboard`, `rename`, `wake`, `hibernate`, `portrait`, close rules, `background`, empty-submit) | new `crates/mx-plugins/src/rpc/dispatch_sessions.rs` (called from `dispatch.rs` match arm fallthrough); request structs + trait methods in `crates/mx-plugins/src/rpc/api.rs` (`send_keys`, `paste_clipboard`, `rename_session`, `wake_session`, `hibernate_session`, `set_portrait`, extend `OpenClaude{background}`, `TabOwner{kind}` for close) |
| `TerminalKey` names table + validation message | `crates/mx-plugins/src/rpc/keys.rs` (new) |
| portrait path containment (realpath under pluginDir/dataDir) | `crates/mx-plugins/src/rpc/portrait.rs` (new; pure fn over `std::fs::canonicalize`) |
| `SessionJson.context` | `crates/mx-plugins/src/host_json.rs` |
| host impl: rename (empty → `custom_label = None`), wake/hibernate guards, close of claude tab, portrait storage, background open (`register(.., place=false)` already exists) | new `crates/mx-store/src/plugins/session_api.rs`; wire in `crates/mx-store/src/plugins/host.rs` |
| events `session.renamed` (in `rename_tab`), `session.status` on hibernate/wake/settle, `session.context` | `crates/mx-store/src/tabs_ops.rs`, `hibernate.rs`, `tabs_open.rs` (`settle_launch`), the context-ingest site in `ingest.rs` |
| `view.closed` direct delivery to owner | `crates/mx-plugins/src/registry/delivery.rs` (`view_closed(plugin, view)`), called from store modal close (`crates/mx-store/src/plugins/views.rs`) |
| `Tab.portrait: Option<String>` (not persisted) | `crates/mx-core/src/model/tab.rs` (exclude from recipe/serde) |
| trust-prompt detection, `screen_text`, `send_keys` with 60 ms gap, `enter()` | `crates/mx-term/src/trust.rs` (new: regex + screen text), `crates/mx-term/src/session.rs` (`send_keys`, `enter`); `paste_clipboard(text, images_via_ctrl_v)` already exists in `session.rs:399` |
| `Term` port additions (`asks_trust`, `send_keys`, `enter`, `paste_clipboard`) | `crates/mx-core/src/ports.rs` + `crates/mx-app/src/terminal_glue.rs` impl + `crates/mx-store/src/testing.rs` fake |
| settle retry loop (2 s × 300 while asks_trust) | `crates/mx-store/src/tabs_open.rs` `settle_launch` (+ `TimerKey::Settle` reschedule) |
| `PreCompact`/`PostCompact` hook registration | `crates/mx-claude/src/launch.rs` `HOOK_EVENTS` (10 → 12, no matcher); reducer no-op check in `crates/mx-claude/src/hook/` |
| portrait alert + body + tap-to-focus | `crates/mx-store/src/ingest.rs` `check_alerts`; `Notifier` port: add `alert_rich(Alert{title, body, subtitle, image, tab_id})` in `mx-core/src/ports.rs`; tap → `Msg::FocusTab` |
| `notify-send -i` command | `crates/mx-sys/src/notify.rs` (`alert_command` gains `image: Option<&str>`) |
| macOS native notification w/ attachment + click | `crates/mx-sys/src/notify_macos.rs` (new; `objc2-user-notifications`), behind `cfg(target_os="macos")`. **Decision**: Rust build is Linux-first (.deb); macOS UN* requires a bundled, signed app id — suggest deferring and always using the osascript fallback on macOS. Optional Linux improvement (not in Dart): `notify-send --action=default=Abrir --wait` to support click-to-focus — flag, off by default to stay faithful. |
| `folders` setting type (resolve/fits) | `crates/mx-plugins/src/manifest/settings.rs` |
| `folders` setting UI row (Todas/Nenhuma, checkboxes, `Nenhuma pasta na lateral`) | `crates/mx-app/src/plugin_ui/settings.rs` (overlap risk with area E settings work — assign to the plugin-UI owner) |
| `PluginKind` + `manifest.kinds()` | `crates/mx-plugins/src/manifest/mod.rs`; tag widget `crates/mx-app/src/plugin_ui/kind_tag.rs` (new); catalog `kinds` parsing goes with area B's catalog module |

#### F.14 Already in Rust / discrepancies

- `session.openClaude` dispatch has no `background`; store `register(tab, term, place)` already supports `place=false`.
- `session.close` (dispatch.rs:200) still 2.4: optional `tabId` via `tab_owner(p, Option)`, owner-only. Must require tabId
  and add the claude+`sessions.close` branch with the two distinct messages.
- `Permission` has 4 variants; `tests/manifest.rs:36` asserts the old list.
- `initialize_params` stops at `editor`; none of the 2.5–2.7 flags.
- `SessionJson` has no `context`.
- `Store::rename_tab` stores `Some("")` for an empty label — Dart resets to default; verify `title()` treats `""` as unset or change to `None`. No `session.renamed` emit.
- `Store::hibernate` / `wake` emit nothing to plugins; `hibernate` silently returns on `!can_hibernate` (fine for UI; the RPC layer must produce the two error messages and the "trabalhando" guard itself).
- `settle_launch` has no trust-prompt retry and does not emit `session.status ready`.
- `HOOK_EVENTS` has 10 entries (no compaction events).
- `mx-sys/src/notify.rs::alert_command` has no image; uses `--` (Dart doesn't); `Notifier` port has only `alert(title, body)`, no tap callback.
- `mx-term` already has `paste_clipboard(text, images_via_ctrl_v)` sending `\x16` for an image — reuse for `terminal.pasteClipboard`; no `send_keys`, no `screen_text`/trust detection.
- `SettingType` lacks `Folders`.



---

# Part 2 — Parallel work packages and file ownership

Six packages. Every file has exactly one owner. Paths are under `crates/` unless absolute or rooted at the repo (`.github/`, `packaging/`, `branding/`, `Makefile`).

Rules that make parallel work safe:

- **New files first.** Each WP puts its logic in new files it owns. It edits shared files only as listed below.
- **Append-only module lines.** In `lib.rs` / `mod.rs` / `main.rs`, any WP may *append* `mod x;` / `pub use` lines. Nothing else in those files changes, so conflicts are trivial line merges.
- **Extend types from your own file.** A WP that needs new methods on a type in a file it does not own adds an `impl Type { … }` block in a new file of its own (Rust allows this inside the same crate). Example: WP6 adds `Session::screen_text`/`send_keys` in `mx-term/src/session_api.rs`, not in `session.rs`.
- **Contested files.** Small edits that another WP needs in a file it does not own are done by the owner. They are listed per WP under "also applies for".
- **Capability flags.** A flag is only advertised once its feature has landed. WP6 owns the flag list; the other WPs tell WP6 when to switch a flag on.

## WP1 — Updates: app updater, plugin catalog, release pipeline, branding (sections A, B)

Owns:
- `mx-core/src/version.rs` (new, `compare_versions`, shared by updater and catalog)
- `mx-core/src/persist/config.rs`, `mx-core/src/persist/read.rs`, `mx-core/tests/persist.rs` (top-level `autoUpdate`)
- `mx-sys/src/updater/{mod,version,sums,release,fetch,install}.rs` (new), `mx-sys/tests/updater.rs` (new), `mx-sys/Cargo.toml`
- `mx-plugins/src/catalog/{mod,fetch,updates,tests}.rs` (new), `mx-plugins/src/registry/{install,persist,mod}.rs`, `mx-plugins/src/manifest/mod.rs` (`kinds()`), `mx-plugins/Cargo.toml`
- `mx-store/src/updates.rs` (new), `mx-store/src/plugin_updates.rs` (new), `mx-store/src/lifecycle.rs` (the quit hook `updater.on_quit()`, run after sessions shut down)
- `mx-app/src/status_bar/update_chip.rs` (new) + one-line insert in `status_bar/mod.rs`
- `mx-app/src/app/update_confirm.rs` (new), `mx-app/src/app/mod.rs` (quit path only)
- `mx-app/src/chrome/settings/{model,mod,dialog}.rs`, `chrome/settings/updates.rs` (new), `chrome/settings/plugins.rs`
- `mx-app/src/plugin_ui/catalog.rs` (new), `mx-app/src/plugin_ui/install.rs` (extract the shared trust dialog)
- `mx-app/assets/icons/*`, `branding/` (new), `.github/workflows/release.yml`, `packaging/linux/make-deb.sh`, `packaging/macos/make-dmg.sh`, `Makefile`
- workspace `Cargo.toml` (add `sha2`)

Also applies for:
- E (TASK#82249): the switch restyle in `chrome/settings/plugins.rs`, using WP5's `MxSwitch`.
- F: the plugin kind tag in the same file (`plugin_ui/kind_tag.rs` comes from WP6).

Blocked on: decision 1 (updater target), for `release.rs`/`install.rs` only. Everything else can start now.

## WP2 — Terminal: reflow copy, sticky selection, find in pane (section C)

Owns:
- `mx-term/src/{reflow,sticky,find}.rs` (new), `mx-term/src/session.rs`, `mx-term/src/io.rs`, `mx-term/src/events.rs`
- `mx-app/src/find_bar.rs` (new), `mx-app/src/terminal_view.rs`, `mx-app/src/terminal_element.rs`, `mx-app/src/terminal_glue.rs`, `mx-app/src/terminal_tests.rs`
- the terminal context-menu `Copiar` reflow flag. If that lives in `app/menu_actions.rs`, WP5 makes the one-line call change.

Exposes `TerminalView::open_find()` for the keymap. Does **not** edit `keymap/*`: WP5 binds the `find` action.

## WP3 — CodeEditor parity (section D §TASK#82253.1)

Owns:
- `mx-rfw/src/widgets/code_editor.rs`
- `mx-app/src/rfw_view/{code_editor,code_pane,code_pane_tests}.rs`
- new submodules under `mx-app/src/rfw_view/code_editor/` (e.g. `find_bar.rs`, `menu.rs`, `fold.rs`), declared from `code_editor.rs` so `rfw_view/mod.rs` is untouched
- `mx-app/src/setup/editor/*` (keys, syntax colours from the ansi palette, element)

First task: the revision/text data-loss bug, plus flushing the buffer when the doc leaves the screen and keeping buffers alive across `Surface` rebuilds.

## WP4 — Other plugin widgets + modal (section D §§.0, .2–.8)

Owns:
- `mx-rfw/src/widgets/{text_input,sprite,zoom_view}.rs` (new), `mx-rfw/src/widgets/maestria.rs`, `mx-rfw/src/tree.rs`
- `mx-app/src/rfw_view/{text_input_widget,sprite,zoom_view,selectable,markdown_widget,anchor}.rs` (new), `rfw_view/{widgets_maestria,mod,view,icon_paths,blocks_list}.rs`
- `mx-app/src/reader/*` (markdown selection)
- `mx-plugins/src/ui/{icons,blocks}.rs`
- `mx-store/src/plugins/views.rs` (data-only redraw, modal `from`/`width`/`height`)
- `mx-app/src/plugin_ui/modal.rs` (grow/shrink 340/260 ms, icon "abrir como painel")

Also applies for F: emitting the `view.closed` event in `views.rs`.

Depends on WP6 for the wire parsing of `view.open` `from`/`width`/`height` in `rpc/api.rs`. WP6 fills the fields of the `ViewOpen` struct and WP4 consumes them. Agree the field names on day one.

## WP5 — App shell: hover, panel hotkeys, pane drag, session cards, history attach, switch, folders picker (section E)

Owns:
- `mx-core/src/model/{tab,agent,group}.rs`, `mx-core/src/persist/pane.rs` (hotkey; drop dirty; `AgentInfo.id`)
- `mx-sys/src/history.rs` (`ChatStanding::Background`)
- `mx-store/src/{tabs_ops,prefs,history_ops,git_ops}.rs`
- `mx-app/src/keymap/*`, including the C3 bindings: `find` on ⌘F / Ctrl+Shift+F, calling `TerminalView::open_find()`; `search` moved to ⇧⌘F / Ctrl+Alt+F; slot system removed
- `mx-app/src/chrome/menu/*`, `chrome/widgets/{controls,mod}.rs` + `chrome/widgets/hover.rs` (new), `chrome/dialog/hotkey.rs` (new), `chrome/settings/{shortcuts,appearance,programs}.rs`, `chrome/notices/*`
- `mx-app/src/app/{drag,chats,menu_actions}.rs`
- `mx-app/src/panes/*` + `panes/{session_cards,plugin_tiles}.rs` (new)
- `mx-app/src/sidebar/*` + `sidebar/glyph.rs` (new), `mx-app/src/rail/*`
- `mx-app/src/plugin_ui/{settings,pick}.rs` + `plugin_ui/folder_picker.rs` (new)

Also applies for:
- F: the `context` field on `MxTab` in `model/tab.rs`, per F.
- B: rail "+" opens WP1's catalog dialog.
- C: the `Copiar` reflow flag if it lives in `menu_actions.rs`.
- F/E: the `folders` setting rendering. 281c232 replaces b043ae0's inline checkboxes with the picker; WP5 builds the picker; the `SettingKind::Folders` type comes from WP6.

Ships `MxSwitch` early (in `controls.rs`) so that WP1 can use it.

## WP6 — Sessions API, notifications, protocol plumbing (section F + protocol parts of D)

Owns:
- `mx-plugins/src/rpc/{api,dispatch}.rs`, `rpc/{dispatch_sessions,keys,portrait}.rs` (new)
- `mx-plugins/src/registry/{lifecycle,delivery}.rs` (**all** `initialize` flags, D's and F's), `mx-plugins/src/host_json.rs`, `mx-plugins/src/permission.rs`, `mx-plugins/src/manifest/settings.rs` (`Folders` type), `mx-plugins/tests/manifest.rs`
- `mx-store/src/plugins/{host,session_api}.rs` (`session_api.rs` new), `mx-store/src/{ingest,tabs_open,hibernate,testing}.rs`
- `mx-core/src/ports.rs`
- `mx-term/src/{trust,session_api}.rs` (new; an `impl Session` block, not edits to `session.rs`)
- `mx-claude/src/launch.rs` (PreCompact/PostCompact hook groups)
- `mx-sys/src/notify.rs`, `mx-sys/src/notify_macos.rs` (optional, see decision 2)
- `mx-app/src/plugin_ui/kind_tag.rs` (new), `mx-app/src/terminal_session_api.rs` (new; any glue that would otherwise go in `terminal_glue.rs`)

Also applies for:
- E (TASK#82252): the `apply_agents` rule in `ingest.rs` (do not overwrite a hook-reported session id) and the "open shell running `claude attach '<id>'`" helper in `tabs_open.rs`. WP5 calls both.
- D: parsing of the `view.open` params.

## Dependency graph

- None of the WPs block each other at the start.
- Interfaces to agree on day one:
  - `compare_versions` (WP1, internal)
  - `MxSwitch` (WP5 → WP1)
  - `TerminalView::open_find` (WP2 → WP5)
  - `ViewOpen` fields (WP6 → WP4)
  - `SettingKind::Folders` (WP6 → WP5)
  - the attach-shell helper (WP6 → WP5)
  - `PluginKind`/`kinds()` (WP1 → WP6 kind tag)
- Suggested merge order: WP6 and WP5 first (they own the most shared files), then WP2, WP3, WP4, and WP1 last (after decision 1).
