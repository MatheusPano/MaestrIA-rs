# 05 — UI shell, theming and packaging

Source of truth: the Flutter app at `/home/lucas/maestria/repo` (version `2.4.0+1`).
Paths below are relative to that repo unless stated. All UI strings are Portuguese and
quoted **verbatim**; keep them byte-identical (including `…`, `—`, `·`, curly quotes `“ ”`).

Scope: the window chrome around the panes (window, status bar, sidebar rail, sidebar,
menus, dialogs, settings, notices/toasts/banner, flow editor), the theme system, assets,
animations, packaging/CI. The pane area itself (`PaneArea`, terminal/reader/plugin panes,
pane header) and plugin RFW rendering are specified elsewhere; they are referenced here
only where the shell touches them.

Notation: `Mx.<token>` = palette token of the current theme (section 2). Sizes are
logical pixels. `store.x()` = an action on the central app store (`lib/services/store.dart`)
— the Rust store must expose an equivalent.

---

## 1. Window / app structure

### 1.1 Process and window

| Item | Value |
|---|---|
| Window title (Material app title, GTK header bar, mac bundle name) | `MaestrIA` |
| Binary name | `maestria` |
| App id (Linux GApplication id, `g_set_prgname`, desktop file name, icon name, `StartupWMClass`) / mac bundle id | `br.com.marrow.maestria` |
| GApplication flags | `G_APPLICATION_NON_UNIQUE` (several instances allowed) |
| Linux default size | 1280 × 720 (`gtk_window_set_default_size`) |
| macOS initial content rect (MainMenu.xib) | 800 × 600 at (335, 390); window class `MainFlutterWindow` |
| Linux decorations | GTK header bar with title `MaestrIA` + close button when running under GNOME Shell (or Wayland); plain title bar on other X11 WMs |
| First paint | window is realized hidden, shown on the first rendered frame (avoid a black flash). View background `#000000` before first frame |
| macOS menu bar | stock: `APP_NAME` (About/Services/Hide/Hide Others/Show All/Quit), `View` (Enter Full Screen), `Window` (Minimize/Zoom/Bring All to Front), `Help` |
| macOS min OS | 12.0 (`MACOSX_DEPLOYMENT_TARGET`) |
| Config file | `~/.maestria/config.json` (store persists UI state, debounced 400 ms) |

Lifecycle hooks (from `main.dart`):

- **Exit requested** (⌘Q, window close) → `await store.shutdown()` (kills every pty
  session so no orphan `claude` survives), then exit.
- **Window resumed / focused** → `store.setWindowActive(true)`; **inactive** →
  `store.setWindowActive(false)`. Used by notices: a pane finishing while the window is
  inactive is "news"; toast auto-dismiss timers are not armed while the window is inactive.
- On startup: pre-warm the two SVG icons before the first frame (otherwise the Claude
  mark appears one frame late and the list flickers), then `store.init()`.

Native bridge (`maestria/dock` method channel) the Rust app must re-implement natively:

| Method | macOS | Linux |
|---|---|---|
| `badge(label)` | `NSApp.dockTile.badgeLabel = label` (empty/null clears). Label = count of sessions waiting on the user | no-op (returns null) |
| `isActive` | `NSApp.isActive` | `gtk_window_is_active` |
| `quickLook(path)` | QLPreviewPanel on the file; returns bool | returns `false` (caller falls back to `xdg-open`) |
| `chooseFolder` | NSOpenPanel sheet, dirs only, prompt `Escolher`, message `Escolha a pasta do repositório` | GTK file chooser SELECT_FOLDER, title `Escolha a pasta do repositório`, buttons `Cancelar` / `Escolher` |
| `chooseWorkspace` | NSOpenPanel sheet, files only, type `.code-workspace`, prompt `Adicionar`, message `Escolha um workspace do VS Code` | GTK OPEN, title `Escolha um workspace do VS Code`, filter name `Workspace do VS Code`, pattern `*.code-workspace`, buttons `Cancelar` / `Adicionar` |
| `chooseMarkdown(startDir)` | NSOpenPanel sheet, prompt `Abrir`, message `Escolha um markdown pra ler`, types plain-text + `net.daringfireball.markdown` (fallback ext `md markdown mdx txt`), starts in `startDir` | not implemented (feature absent on Linux) |

All pickers are **modal to the main window and asynchronous** (never spin a nested loop
that stops rendering). A picker that cannot open returns "unavailable" and the caller shows
the sticky banner `Não consegui abrir o seletor de arquivos`.

### 1.2 Top-level layout (`main.dart`)

```
Window (Mx.canvas background)
├─ [StatusBar]            ← here if store.statusBarTop
├─ Expanded Stack
│   ├─ Padding(L=gap, T=top?0:gap, R=gap, B=top?gap:0)
│   │   └─ Row
│   │       ├─ SidebarRail (36 wide + 8 right gap)       always present
│   │       ├─ if !sidebarHidden: Sidebar (width = clamp(store.sidebarWidth, 248, max(248, windowWidth-280)))
│   │       ├─ if !sidebarHidden: SidebarGrip (8 wide)
│   │       └─ Expanded Stack
│   │           ├─ PaneArea (out of scope)
│   │           ├─ if noticesOpen: NoticeCenter at right=8, top=8 (bar on top) or bottom=8
│   │           └─ if !noticesOpen: column aligned bottom-right (or top-right) at L=R=8:
│   │                 [banner if top] · NoticeToasts · [banner if bottom]
│   ├─ if sessionsOpen: SessionList at left=StatusBar.inset (52), top/bottom 8
│   ├─ PluginFloatLayer (fills; plugin floating panels)
│   └─ PluginModalLayer (fills; plugin modal, above everything)
└─ [StatusBar]            ← here if !statusBarTop (default)
```

Constants: `Mx.gap = 8`, `Mx.radius = 10`. Hidden sidebar is **removed from the tree**,
not zero-width. The banner and toasts float over the panes (never take layout height —
resizing the pane area would resize every pty).

Keyboard: a root focus node above everything receives key events bubbling from wherever
focus is (terminal views do not claim ⌘ combos) and dispatches the keymap
(`MxKeys.dispatch`); a second always-present focus node routes plugin key bindings
(`store.plugins.commandFor(event, except: keymap)` → `store.runPluginCommand`). Details of
the keymap belong to the shortcuts spec; the default table is reproduced in 1.10.

#### 1.2.1 SidebarGrip

8 px wide gutter between sidebar and panes, cursor `resizeLeftRight`. Centered handle
3 × (hover/drag ? 46 : 0) px, radius 2, color `Mx.accent` while dragging else `Mx.fgFaint`,
height animates 120 ms. Drag → `store.setSidebarWidth(width + dx)` (clamped 248…680,
persisted). Double-click → `store.setSidebarWidth(352)` (default).

#### 1.2.2 Banner ("recado")

- Shown when `store.banner != null`. `store.showBanner(text, sticky=false)`: replaces any
  previous banner and restarts the timer; non-sticky banners clear after **6 s**
  (`bannerLife`); sticky ones wait for the close button. A new non-sticky banner after a
  sticky one replaces it and does expire.
- Card: max width 460, bg `Mx.bgSidebar`, radius 8, border `accent @ 0.7`, shadow
  (`Mx.shadow`, blur 18, offset 0,6), padding 14/12/8/12. Row: `Icons.info_outline` 16
  `Mx.accent` (top-padded 1) · 10 · text 12.5 / line-height 1.4 `Mx.fg` (wraps) · 12 ·
  close button (20×20, radius 4, hover bg `Mx.bgHover`, `Icons.close` 14, `Mx.fg` on hover
  else `Mx.fgDim`) → `store.clearBanner()`.
- Entry animation: 160 ms easeOut, opacity 0→1 and translateY 8→0. Keyed by text, so a new
  text replays the entry.

### 1.3 Status bar (`ui/notices.dart` `StatusBar`)

Height 26, full window width, on the canvas (not a card). Padding: left `inset = 8 + 36 + 8
= 52`, right 12, 3 px towards the window edge. Row: `_SessionSummary` (left) · spacer ·
`NoticeBell` (right). Position toggled by settings (`store.statusBarTop`, default bottom).

**Session summary** (pill 20 high, h-pad 6, radius 4, hover/open bg `Mx.bgHover`):
rich text 11.5, line-height 1, color `Mx.fgDim` when hover/open else `Mx.fgFaint`:
`"{n} sessão" | "{n} sessões"` + (if working>0) `"  ·  {w} trabalhando"` + (if waiting>0)
`"  ·  {k} esperando você"` in `Mx.yellow`. Counts over live Claude tabs (`kind==claude &&
!exited`); working = status `working|tool`; waiting = `!done && status.needsHuman`.
Tap → `store.toggleSessions()`. Belongs to tap group "sessions".

**Session list** (`SessionList`, popover): width 420, max height 520, bg `Mx.bgSidebar`,
radius 8, border `Mx.border`, shadow blur 18 off (0,6). Enter: 160 ms easeOut, fade + slide
from ±8 (from above if bar on top). Tap outside the group → `store.closeSessions()`.
Empty: `Nenhuma sessão do Claude aberta` (12, fgFaint, padding 16). Otherwise blocks in
order, each with a caption `"{label}  {count}"` (10.5, w600, letterSpacing 0.4, fgFaint,
padding 10/12/10/6): `Esperando você`, `Trabalhando`, `Abertas`. Row: margin v1, radius 6,
bg `bgActive` if it is the focused pane else hover `bgHover`, padding 10/9/12/9:
`ClaudeAvatar(size 20)` · 12 · [title 13/1.3 w600 fg, 1 line; 2; `"{folder} · {subtitle}"`
(folder omitted for loose) 11.5/1.3 fgFaint] · 10 · status label 11.5 in status color.
Tap → `store.openSession(tab)`.

**Notice bell**: pill 20 high, h-pad 6, radius 4, bg `bgHover` on hover or when open.
Icon 15: `notifications_off_outlined` if do-not-disturb, `notifications_none_outlined` if
0 unread, else `notifications_outlined`; then count (11, w700, `"99+"` above 99). Color
`Mx.accent` when open or unread>0, else `fg` hover / `fgDim`. Tooltip joined by `" · "`:
`Notificações` [+ `"{n} nova"`/`"{n} novas"`] [+ `não perturbe`]. Tap →
`store.toggleNotices()`. Sessions list and notices list are mutually exclusive.

### 1.4 Notices (bell list, toasts)

Store model: `MxNotice{tabId, kind, title, at, suggestionId?, detail?, read}`; list capped at
**50**, newest first. A new non-`suggested` notice from a tab marks that tab's previous
notices read. A notice for a tab the user is watching enters already read. Toast shown only
if `!read && !doNotDisturb && !noticesOpen`; at most **3** toasts (oldest dropped); each
toast auto-dismisses after **8 s** (`toastLife`), timer paused while hovered
(`store.holdToast(n, true/false)`) and not armed while the window is inactive. Opening the
list clears all toasts.

Kinds:

| kind | verb (after title) | color | icon |
|---|---|---|---|
| question | `te fez uma pergunta` | yellow | `question_mark_rounded` |
| permission | `quer aprovação` | red | `lock_rounded` |
| finished | `terminou o trabalho` | green | `check_rounded` |
| suggested | `sugeriu uma tarefa` | accent | `lightbulb_outline_rounded` |

**NoticeCenter** (list popover): width 380, max height 420, bg `bgSidebar`, radius 8,
shadow blur 18 (0,6), foreground outline `accent @ 0.7`. Same 160 ms enter as session list.
Tap outside → `store.closeNotices()`.
Header (h 36, padding L14 R6, bottom border): title `notificações` (12.5 w700 fg) · four
22×22 small buttons (icon 15, radius 4, hover `bgHover`; disabled = `fgFaint @ 0.5`):

| icon | tooltip | action |
|---|---|---|
| DND on: `notifications_off_rounded` (accent) / off: `notifications_off_outlined` | on: `Não perturbe ligado · os avisos não aparecem na tela`; off: `Não perturbe · avisos só aqui, sem aparecer na tela` | `store.toggleDoNotDisturb()` |
| `done_all_rounded` | `Marcar tudo como lido` | `store.readAllNotices()` (disabled if no unread) |
| `clear_all_rounded` | `Limpar tudo` | cascade clear (below), disabled if empty |
| `expand_more_rounded` | `Fechar` | `store.closeNotices()` |

Empty text (padding 14/6/14/18, 12, fgFaint, h 1.4):
`Nenhuma notificação — quando um painel perguntar algo ou terminar, aparece aqui`.

Row (`_NoticeRow`, used by list and toasts): rows separated by 1 px `border` dividers;
padding 12/10/6/10; hover bg `bgHover` (not if the tab is gone). Leading 20×20 circle,
bg `kind.color @ 0.16`, icon 12 `kind.color`. Body (opacity 0.6 if the tab no longer
exists): rich text 12.5/1.35, color `fg` if unread else `fgDim`: **`{current tab title}`**
(w700 fg; falls back to the stored title) + `" {verb}"` + optional `": {detail}"` (w600 fg);
then 3 px; meta line 11 fgFaint, joined `" · "`: folder name (non-loose), `painel fechado`
if gone, `shortAgo(at)`. Trailing 22×22: on hover or in a toast → close button (tooltip
`Tirar da tela` in toast / `Dispensar` in list); else an unread dot 7×7 `accent`.
Tap → `store.openNotice(n)` (focuses the pane, marks read, removes toast); if the notice
carries a suggestion → also open the suggestion dialog (`showSuggestion`, other spec).

`shortAgo`: `<60s → "agora"`, `<60min → "{m}min"`, `<24h → "{h}h"`, `<7d → "{d}d"`, else
`"{day}/{month}"`.

**NoticeToast**: same card as the list (width 380, radius 8, outline accent@0.7, shadow),
8 px gap from neighbour (below if bar bottom, above if top). Toasts column order: oldest
nearest the bar.

### 1.5 Sidebar rail (`ui/sidebar_rail.dart`)

Column of 36×36 icons, spacing 4, to the left of the sidebar, on the canvas (stays visible
when the sidebar is hidden — it is the way back). Each icon: radius 8; selected bg
`bgSidebar` + border `border`; hover bg `bgHover`; icon color `fg` when lit else `fgFaint`
("faint" variant: `fgDim` hover else `fgFaint @ 0.55`). Tooltip waits 400 ms, above.
Badge at right 1 / bottom 1.

1. **Sessões** (`space_dashboard_outlined` 18). Selected when the session list is shown.
   Tooltip `Sessões` or `"Sessões — {n} esperando por você"`; when it is the current view
   the tooltip is `Esconder a lateral` + shortcut labels (joined by two spaces) — exact:
   `["Esconder a lateral", ...keys].join("  ")`. Badge (only when not selected and waiting
   > 0): text = waiting count, color = status color of the first waiting session.
   Tap → `store.showSidebarView(null)` (tapping the current view hides the sidebar).
2. **One per rail plugin** (`store.railPlugins`), drag-reorderable
   (`store.moveRailPlugin(id, toIndex)`): plugin glyph 18. Tooltip `"{name}"` or
   `"{name} — {n} janela aberta"`/`"… janelas abertas"`. Badge priority: busy (any command
   of the plugin running) → 8 px dot `accent` with 1.5 px `canvas` border; else
   plugin-provided text badge (`store.sidebarBadges[id]`) on `bgActive`/`fg`; else open
   window count on `bgActive`/`fgDim` (only when not current). Tap →
   `store.showSidebarView(pluginId)`.
3. **Instalar plugin** (`add` 16, faint) → install-plugin dialog (plugin spec).

Text badge: height 14, min width 14, **max width 32 (= 36 − 4)**, h-pad 3.5, radius 7,
1.5 px `canvas` border, text 8.5 w700 line-height 1, clipped, no wrap; text color given or
auto (`canvas` if bg luminance > 0.5 else white).

### 1.6 Sidebar (`ui/sidebar.dart`, `sidebar_workspaces.dart`)

Card = `MxPanel(color: bgSidebar)`: radius 10, shadow (`Mx.shadow`, blur 12, offset 0,3),
1 px `border` outline drawn on top (focus ring variant: `accent@0.65` 1.4 px; tinted
variant: tint 2 px).

Two modes:

#### A. Session list mode (default)

**Header** (h 42, h-pad 8, bottom border): search field (expanded) · gear icon
(`settings_outlined`, tooltip `Configurações` + keys) → `showSettings()`.

Strip icons (`_StripIcon`): 36×36, glyph 18 `fgDim`, splash radius 16, gap 4.

**Search field**: h 28, padding L8 R2, radius 8, bg `bgActive` focused / `bgHover`; border
`accent` focused / `border`. `search` icon 14 (fgDim focused / fgFaint) · 6 · text field
12 fg, cursor accent 1.5; hint `Buscar sessão` when focused else `"Buscar sessão  {first
search key label}"` (hint 12 fgFaint). Clear mini-button (`close_rounded`, tooltip
`Limpar a busca`) when query not empty → `store.setQuery('')`. Filter mini-button
(`filter_list_rounded`, accent when filters active) tooltip `Filtrar por pasta,
feature/hotfix ou estado` or `"{n} filtro — clique pra mexer"` / `"{n} filtros — …"` →
filter menu. Mini-button: 22×22 radius 6, hover bg `border`, icon 14. Typing →
`store.setQuery(text)`. Esc: if filtering → `store.clearSearch()` else unfocus. The search
shortcut focuses this field (and first un-hides the sidebar).

**List** (padding top 6, bottom 20; keep all rows laid out — reveal scroll needs them),
in order:

1. **Groups tray** (if groups exist and not filtering): ruler row (chevron 14 fgFaint +
   `Grupos` 10.5 fgFaint ls 0.5 · line `border` · count 11 · hover-only clear button
   tooltip `Esquecer todos os grupos` → `confirmClearGroups`). Tap ruler →
   `store.toggleGroupsCollapsed()` (persisted `groupsCollapsed`). Rows (`_GroupRow`):
   leading `grid_view_rounded` 17 in group color, title = name, subtitle = `group.where`
   if name == summary (case-insensitive) else `group.summary`; trailing count chip + ⋯
   (tooltip `O que fazer com esse grupo`). Tap → `store.openGroup(g)`; right-click/⋯ →
   group menu. Selected when the screen *is* this group (`store.showing(g)`).
2. `_NoFolders` (no folders, not filtering), 12.5/1.55 fgFaint, padding 20:
   `Nenhuma pasta ainda.\n\nSessões do Claude já rodando aparecem aqui sozinhas — ou use o ícone de pasta no rodapé pra adicionar um repo.\n\nPra abrir um painel sem pasta nenhuma, o + na linha de "avulsos" logo abaixo.`
3. `_NoHits` (filtering, no hits): `Nenhuma sessão com {what}.` where what = `“{query}”`
   and/or `"{n} filtro(s)"` joined `" + "`; button `Mostrar tudo`
   (`filter_list_off_rounded`, tooltip `Limpar a busca e os filtros`) →
   `store.clearSearch()`.
4. For each `store.sidebarRows` (ordered root rows): Workspace section or Folder group
   (hidden when filtering and no hits inside).
5. **Loose tray** ("avulsos", `store.loose`): ruler `{loose.name}` + line + badge + count
   + hover clear-settled (`Limpar 1 painel concluído` / `Limpar {n} painéis concluídos`) +
   `+` + ⋯ (`O que fazer com os avulsos`: `Nova feature…`, `Novo hotfix…`,
   `Limpar concluídos`). Top padding 22 (4 if no folders). Then its features/hotfixes and
   tab rows.

**Workspace section**: header row (padding 10/8/8/8, top margin 6): chevron (20 fgDim;
`chevron_right` collapsed / `expand_more`) · `hexagon_outlined` 15 (workspace tint or
accent) · name 13.5 w600 · [needs-human badge if collapsed] · `"1 pasta"`/`"{n} pastas"`
11.5 fgFaint · ⋯ (`O que fazer com esse workspace`). Tap → toggle collapsed; right-click/⋯
→ workspace menu. Children nested (`_Nest`: left margin 20, left 1 px line in workspace
tint or `border`): folder groups (a folder may appear in several workspaces — "mirrored";
collapse state is per appearance) or `Arraste uma pasta pra cá` (11.5 fgFaint) when empty.

**Folder group**: header (padding 10/8/8/8, top 6, hover-aware): chevron · `RepoGlyph`
(repo: `folder_rounded` + small `call_split` badge in purple on a `bgSidebar` circle;
non-repo: `folder_outlined`; color = folder tint ▸ workspace tint ▸ default) · 9 · name
13.5 w600 + optional branch line (`call_split` 10 + branch mono 10.5 fgFaint) · ghost chip
(prunable worktrees; yellow@0.14 bg, `link_off` 11, count 10 w700; tooltip `Uma sem pasta
no disco` / `"{n} sem pasta no disco"`) · needs-human badge · tab count 11.5 · `+`
(visible on hover or when the folder has no tabs; 110 ms fade) · ⋯ (`O que fazer com essa
pasta`). Tap → `store.toggleFolderCollapsed(folder, within)`. Children (`_Nest` rail 20):
features/hotfixes, then tab rows not in a feature, then 8 px.

**Feature/hotfix group**: header (padding 7/8/4/8, margin h7, radius 8; is a drop target
for tabs of the same folder → `store.assign(tab, f)`, hover bg `bgHover` + accent border
while dragging over): chevron 16 · kind icon 15 (`track_changes` feature, `bolt` hotfix;
tint or purple) · name 12.5 w600 · `hotfix` (10.5 fgFaint, hotfixes only) · brief icon
(`sticky_note_2_outlined` 13, tooltip = brief) · badge · count 11 · `+` · ⋯
(`"O que fazer com {essa feature|esse hotfix}"`). Tap → `toggleFeatureOrHotfixCollapsed`.
Children nested with rail 15.

**Tab row** (`_Row`): margin 7/3, padding 11/11/9/11, radius 9.
Background: grey = selected ? (focused ? `bgActive` : `bgHover`) : hover ? `bgHover` :
transparent; if a tint applies: `alphaBlend(tint @ a, grey or bgSidebar)` with
a = chosen tint ? (selected 0.40 / else 0.26) : group tint ? (0.20 / 0.11).
Leading box 34 wide: Claude → `ClaudeAvatar(size 26, dim: done)`; reader →
`article_outlined` 18; setup → `tune` 18; plugin → plugin glyph; shell → launcher icon 17
in launcher color, else `terminal.svg` 22 fgDim.
Title 14.5, `fgDim` if dim (done, or rested & seen, or hibernated) else `fg`, w600 when
selected. Subtitle 12 fgFaint 1 line + optional stamp (`restedAgo`, 11; green w600 if
unseen).
Trailing (replaced by a close button on hover — 22×22 radius 6, `close_rounded` 15,
tooltip `Fechar painel` 600 ms → `store.closeTab`): `task_alt` 14 green (`Concluída`) ·
follow-up mark · `bedtime_outlined` 13 (`Hibernada — sem processo; clique pra retomar a
conversa`) · yellow dot 6 if dirty · status badge (count 1, status color) if needs human ·
slot label (first 9 tabs: `⌘1`…`⌘9` mac / `Alt+1`… linux; mono 10.5) · status dot 9 for
shells.
Tap → `store.select(tab)` or `store.openGroup(group)` if grouped. Right-click → panel menu.
Rows of a flow's spawned children ("brood": contiguous following tabs with
`descendsFrom(tab)`) are nested one level (rail 18).
Drag & drop: rows draggable within the same folder (`store.moveTab(from, to)`), 2.5 px
accent insertion stripe (inset 7), dragged copy at 0.3 opacity, lifted feedback (bg
`bgSidebar`, radius 12, border accent@0.55, shadow blur 18 off 0,8). Folder/workspace
headers draggable too (`_RowDrag`): dropping a folder in the middle 50% of a workspace
header puts it **into** the workspace (accent@0.12 fill + 1.5 accent border, radius 6);
near the edge it reorders; dropping on a root row takes it out of the workspace;
`store.canDrop / store.drop(from, place, into:)`; dropping on itself toggles collapse.

Reveal ("mostrar na lateral", `store.revealTab(tab)`): un-hide sidebar, switch to session
view, clear the search if it hides the tab, expand the workspace (first one in sidebar
order containing the folder), folder and feature; set `revealedTabId/Within` for **2 s**;
the row consumes `takeRevealScroll` once and scrolls itself into view (alignment 0.3,
250 ms easeOut). Revealed row gets the same accent@0.12 + 1.5 accent border overlay.

Badges: `_Badge` pill (margin R6, padding 5.5/1.5, radius 8, text 10 w700, color default
`red`, text auto contrast). `_CountChip`: padding 5/1.5, radius 6, bg `bgActive`, text 10
fgDim. Row buttons `_RowButton`: icon 18 fgFaint, padding 3, radius 5, tooltip 400 ms,
menu anchored at bottom-left.

**Footer** (h 42, h-pad 7, top border): `create_new_folder_outlined` (`Adicionar pasta ou
workspace`) → footer add menu · `history_outlined` (`Retomar uma conversa`) →
`showChatHistory` · plugin footer commands (spinner if busy) · spacer · account icon
(`speed_outlined`, tooltip `Conta & uso`) → settings at Account · `view_sidebar_outlined`
(`Esconder a lateral` + keys) → `store.toggleSidebar()`.

#### B. Plugin view mode (`store.shownPlugin != null`)

Header (h 42, padding L16 R8): plugin glyph 15 fgDim · name 12.5 w600 · gear
(`Configurar os plugins`) → settings at Plugins. Body: if the plugin declares its own
sidebar (`manifest.sidebar`): optional `Janelas` ruler + rows, then the plugin's own pane
(or `Abrindo {name}…` with a 12 px spinner; crashed: `"{name} caiu: {crash}"` in red +
`Reiniciar` → `store.plugins.restart`). Otherwise: ruler `Janelas` (count, hover clear
`Fechar a janela` / `"Fechar as {n} janelas"`), rows or `Nenhuma janela aberta`; then
ruler `Comandos` + command rows (h 30, radius 6, hover `bgHover`; 16 px icon/spinner/dot;
title 12.5 with `"{Plugin}: "` prefix stripped; key label 11) → `store.runPluginCommand`.
Footer same as mode A.

### 1.7 Menus (`ui/menus.dart`)

Context menu look (all menus): bg `bgActive`, radius 10, 1 px `border`, elevation 12 with
`Mx.shadow`, vertical padding 5, width min 216 max 340. Item row h 36 (48 with subtitle),
h-padding 14: glyph slot 16 · 9 · label 12/1.2 (disabled `fgFaint`, disabled glyph opacity
0.45) [+ 2 + subtitle mono 10.5 fgFaint] · [chord 11 fgFaint, 10 gap] · [trailing, 8 gap].
Divider: height 9, `border`, indent 14 both sides. Positioned at the click/anchor point.

Submenu (`MxSubmenuItem`): row with trailing `chevron_right` 14; opens on hover **or**
click; overlay panel width 216 at the parent row's right edge −4 (flips left if no room),
top = row top −5; clamped 8 px inside the window; height = rows×36 + dividers×9 + 10,
scrolls if taller than window−16. Hovering any normal item closes the open submenu; the
submenu closes with its parent menu. Picking a sub-item pops the parent menu with that value.

Menus and their store actions (labels verbatim, in order):

**"Open here" block** (`openHereItems`), prepended to folder, feature, worktree, add menus:
`Sessão do Claude` (Claude mark 13) → `store.openClaude(folder, cwd, label?, feature?)`;
`Terminal` (`terminal`) → `store.openShell`; `Montar um fluxo…` (`account_tree_outlined`
purple) → flow editor (new); `Abrir um markdown…` (`description_outlined`) →
`store.openMarkdown(folder, cwd, feature)`; then either `Criar um programa…` (no
launchers) → new-launcher dialog then `store.openLauncher`, or submenu `Meus programas`
(`apps_outlined`) listing launchers + divider + `Outro programa…`.

**Footer add**: `Adicionar pasta…` → add-folder dialog; `Importar .code-workspace…` →
picker → `store.importWorkspace(path)`; — ; `Novo workspace…` (hexagon accent) →
new-workspace dialog.

**Folder `+`** (`_showAddMenu`): open-here block; if not inside a feature: — +
`Feature…`, `Hotfix…` (capitalized kind label + `…`) → new feature/hotfix dialog.

**Folder menu** (⋯ / right-click): open-here · — · `Configuração do Claude` (`tune`) →
`store.showSetup(folder)` · `Worktrees` (only if non-main worktrees; trailing count chip,
ghost chip, chevron) → worktrees menu · — · `Nova feature…`, `Novo hotfix…` · `Renomear` →
prompt `Renomear pasta`/`Nome` → `store.renameFolder` · tint submenu → `store.setFolderTint`
· submenu `Adicionar a workspace` (each workspace with checkbox glyph → add/remove; divider;
`Novo workspace…` → dialog preselecting this folder) · `Tirar deste workspace` (only
inside a workspace) → `store.removeFromWorkspace` · — · `Limpar concluídos` →
`store.closeSettled` · `Remover pasta` (red) → `store.removeFolder`.

**Worktrees menu** (anchor +14,+6): one item per worktree: glyph `link_off` (prunable,
yellow) / `home_outlined` (main) / `call_split`; label folder name (main) or short label;
subtitle `Sem pasta no disco` (yellow) / `"{branch} · sessão aberta"` / `{branch}`; if
ghosts: — + `Limpar worktrees fantasmas` (yellow) → `store.pruneWorktrees`. Picking one →
worktree menu (anchor +28,+12).

**Worktree menu**: (non-ghost) open-here (cwd = worktree path, label = short label unless
main) · — · `Abrir no vscode` → `store.openInEditor(path)` · `Configuração do Claude` ·
always `Copiar caminho` → clipboard + banner `Caminho copiado` · non-main: `Excluir
worktree…` / ghost `Limpar registro…` (red) → confirm dialog.

**Workspace menu**: `Renomear…` → prompt `Renomear workspace` · tint submenu → —
· `Associar .code-workspace…` (no file) → picker → `store.linkCodeWorkspace(w, path)` |
(file linked) `Abrir no vscode`, `Desassociar .code-workspace` · — · `Desfazer workspace`
→ `store.dissolveWorkspace` + banner `Workspace "{name}" desfeito — as pastas continuam na
lateral` · `Fechar workspace…` (red) → confirm.

**Feature/hotfix menu**: open-here · — · (non-loose) `Configuração do Claude`, `"Nova task
{nessa feature|nesse hotfix}…"` → new-task dialog · `Briefing…` · `Renomear` (prompt
`"Renomear {feature|hotfix}"`) · tint submenu · `"Virar {other kind label}"` →
`store.setFeatureOrHotfixKind` · — · `"Concluir {kind}"` (`task_alt` green) → confirm ·
`"Dissolver {kind}"` (red) → `store.removeFeatureOrHotfix` + banner `"{Feature
dissolvida|Hotfix dissolvido} — os painéis continuam abertos {nos avulsos|na pasta}"`.

**Group menu**: `Guardar a tela de agora aqui` → `store.saveGroup(name)` + banner
`Grupo "{name}" agora é {summary}` (or `Não há nada na tela pra guardar no grupo
"{name}"`) · `Renomear` → prompt `Renomear grupo` · `Esquecer o grupo` (red) →
`store.removeGroup`.

**Panel menu** (`showPanelMenu`, right-click on a tab row):
1. launcher tab that exited: `"Rodar {launcher} de novo"` → `store.relaunch` · —
2. hibernated: `Retomar a conversa` (`play_arrow` accent) → `store.wake` · —
3. `Renomear…` → prompt `Renomear painel` / `Título` → `store.renameTab`
4. tint: if the feature has a tint → disabled `"Cor: {tint} — {da feature|do hotfix}"`;
   else tint submenu → `store.setTabTint`
5. Claude tabs: `Ver o plano` / `"Ver o plano ({n})"` (disabled if none) →
   `store.showPlan`; `Ver o último recado` (disabled if empty) → `store.showMessage`;
   `Quando terminar…` / `"Quando terminar… ({n})"` → flow editor
6. — ; if open: `Prender o painel`/`Soltar o painel` (chord pinPane) → `store.togglePin`
7. `Agrupar painéis` (paneCount > 1, open, no special screen, not already showing its
   group) → `store.groupPanes()` + banner `{n} painéis agrupados como "{name}"`
8. grouped: `Desagrupar "{name}"` → `store.ungroup`
9. non-passive: `Mover pra feature/hotfix…` → move dialog; `Marcar como concluída` /
   `Reabrir: não está concluída` → `store.setDone` (+ confetti puff at click; banner
   `"{title}: concluída — {n} passo(s) da fila foram descartados"` if queue dropped)
10. plugin commands exist: — + submenu `Plugins` → `store.runPluginCommand(c, on: tab)`
11. — ; `Hibernar (libera a memória)` if `store.canHibernate` → `store.hibernate`;
    close (red, chord closePane): `Fechar o leitor` / `Fechar a configuração` /
    `Fechar a janela do plugin` / `Fechar painel` → `store.closeTab`.

**Terminal context menu**: `Copiar` (chord copy, disabled without selection), `Colar`
(first paste chord).

**Tint submenu** (`tintItem`): row label `Cor` or `"Cor: {label}"`, glyph circle 12
(outlined fgDim when none). Items: `Sem cor` (check_circle_outline when none) then divider
then the 13 tints (glyph filled circle 13 in tint color; `check_circle` for current).
Values `tint:none`, `tint:<name>`.

**Filter menu** (a menu hosting a sheet, 226–320 wide, live-updating): section `Onde`
(10, ls 0.6, fgFaint): each folder (folder icon) + its features (indent 14, purple
kind icon), loose (`inbox_outlined`) + its features; then per group `Tipo`:
`Sessões do Claude`, `Terminais`; `Estado`: `Esperando você`, `Rodando`, `Paradas`,
`Concluídas`. Rows: checkbox icon 15 (accent when on), label 12. Actions:
`store.toggleFilterFolder`, `toggleFilterFeatureOrHotfix`, `toggleFilter`. If filtering:
separator + `Mostrar tudo de novo` → `store.clearSearch()` and close.

### 1.8 Dialogs (`ui/dialogs.dart`, `flow.dart`)

Common: Material AlertDialog look — bg `bgSidebar`, radius 12, 1 px `border`, content
clipped; title 15; fields dense outlined, label 12 fgDim, hint 12 fgFaint, text 13; hints
under fields 11 fgFaint. Buttons: `Cancelar` (text) + primary filled (accent; text
`canvas` if accent luminance > 0.5 else white; destructive = red bg, completion = green).
Enter in the first field submits.

| Dialog | Title | Width | Fields / content | Buttons → action |
|---|---|---|---|---|
| promptText | given | — | 1 field (label given), autofocus | `Cancelar` / `Ok` → returns text |
| Add folder | `Adicionar pasta` | 460 | `Caminho do repo` hint `~/repos/...`; `Escolher pasta…` (`folder_open`), `Escolher workspace…` (hexagon); note `Um .code-workspace adiciona todas as pastas dele de uma vez.` or (yellow) `Não consegui abrir o seletor — cole aí em cima o caminho do .code-workspace` | `Cancelar` / `Adicionar` → `.code-workspace` path ? `store.importWorkspace` : `store.addFolder` |
| Loose in | `Abrir sem pasta` | 460 | `Pasta` hint `Onde o painel vai rodar` (prefilled loose root); `Escolher pasta…`; note `O painel fica em "avulsos" — nenhuma pasta é adicionada.` | `Cancelar` / `Terminal` / `Sessão do Claude` → validate dir exists (else sticky banner `Pasta não encontrada: {path}`) → `store.openClaude/openShell(loose, cwd)` |
| New task | `Nova task em {feature or folder}` | 520 | `id da task` (hint `47730`); row `Branch` (default `feature/TASK#{id}`) + `Pasta da worktree` (default `TASK-{id}`); `Base` hint `Vazio = origin/HEAD`; `Comando pós-criação` hint `Ex: flutter pub get`; note `{id} é substituído. O "#" sai do nome da pasta e fica na branch.` | `Cancelar` / `Criar e abrir` → `store.newTask(...)` |
| Remove worktree | `Excluir {label}?` / `Limpar {label}?` | 470 | path (mono 11 fgFaint); facts (icon 14 + text 12 colored): dirty count (`Nada não commitado` green / `1 arquivo não commitado — isso se perde` / `{n} arquivos não commitados — isso se perde` yellow / `Não deu pra ler o git status — pode haver trabalho não commitado`), unmerged (`Todo commit daqui já está em {base}` / `1 commit que não está em {base}` / `{n} commits que não estão em {base}` red / `Sem base pra comparar os commits`); ghost: `A pasta já não existe — isso só limpa o registro que sobrou no git`; checkbox `Apagar também a branch {branch}` | `Cancelar` / red `Excluir` or `Excluir de qualquer forma` (risky) → `store.removeWorktree(force, forceBranch, deleteBranch)` |
| New workspace | `Novo workspace` | 480 | `Nome` hint `ATRIUM`; checkbox list of folders (max 320 h) | `Cancelar` / `Criar` → `store.createWorkspace(name, folders)` |
| Close workspace | `Fechar {name}?` | 470 | path; facts `1 pasta sai da lateral`/`{n} pastas saem da lateral`; green `Nada é apagado do disco — nem os repos, nem o .code-workspace`; yellow `1 sessão aberta é encerrada`/`{n} sessões abertas são encerradas` | `Cancelar` / red `Fechar workspace` → `store.closeWorkspace` |
| Clear groups (≥2) | `Esquecer os {n} grupos?` | 440 | facts: green `Nenhum painel fecha — a tela de agora fica exatamente como está`; `O que se perde são os {n} arranjos guardados, com os nomes deles`; `Pra ter um de volta: arrume a grade e agrupe os painéis outra vez` | `Cancelar` / red `Esquecer` → `store.clearGroups()` + banner `{n} grupos esquecidos — nenhum painel foi fechado`. With exactly 1 group: no dialog, banner `Grupo "{name}" esquecido — os painéis continuam abertos` |
| New feature/hotfix | `{Nova feature|Novo hotfix} em {folder}` | 560 | `Nome` hint `Permissão do google`; `Briefing (opcional)` (3–8 lines) hint `O que todo agente {dessa feature|desse hotfix} precisa saber antes de começar`; note `O briefing entra como --append-system-prompt em toda sessão aberta aqui dentro — vale a conversa inteira, não só a primeira mensagem.` | `Cancelar` / `Criar` → `store.addFeatureOrHotfix(folder, name, brief, kind)` |
| Brief | `Briefing de {name}` | 560 | `Contexto` (6–14 lines) hint `O que já foi decidido, o que não encostar`; note `As sessões que já estão abertas não mudam — o briefing entra na linha de comando, então vale a partir da próxima que você abrir aqui.` | `Cancelar` / `Salvar` → `store.editFeatureOrHotfix(brief)` |
| Complete feature | `Concluir "{name}"?` | 460 | facts: `Não tem painel aberto {nessa feature}` / `Fecha {n} painel(is) — as sessões terminam aqui`; yellow `{n} painel(is) ainda no meio de um turno`; red `{n} painel(is) esperando você responder`; `O briefing vai junto — copie o texto antes se for reaproveitar`; `Nada é mexido no repo: {a feature|o hotfix} só existia aqui` | `Cancelar` / green `Concluir` or `Concluir de qualquer forma` → `store.completeFeatureOrHotfix` + **Confetti.fire** + banner `"{name}" concluído 🎉` [+ ` — {n} painel(is) fechado(s)`] |
| Move to feature | `Mover "{title}" pra…` (SimpleDialog) | — | one option per feature (kind icon, accent if current), then `Nenhum — solto na pasta` / `Nenhum — solto na bandeja`. If none exist: no dialog, banner `Essa pasta ainda não tem feature nem hotfix — crie pelo + da pasta` / `Os avulsos ainda não têm feature nem hotfix — crie pelo + da bandeja` | → `store.assign(tab, feature or null)` |
| Chat history | `Todas as conversas` | min(760, w−112), list max min(620, h−260) | spinner; empty `O Claude não guardou nenhuma conversa ainda.`; sections per folder (ruler name + count) and loose; rows: Claude mark 13 (or `link_off` yellow when gone), title 12.5, meta mono 10.5 `"{ago} · {where} · {size}"`, standing note (green on screen, yellow live/gone, `Não sei onde ela rodou`); per-folder `Todas as conversas em {folder}` → loads deeper (`Buscando…`) | `Fechar`; tap row → `store.resumeChat(chat)` |
| Launcher form | `Novo programa` / `Editar programa` | 460 | `Comando` (mono) hint `Btop, lazygit, npm run dev…` + note `Roda no painel como você digitaria no terminal — e o painel abre já dentro dele, sem prompt no caminho.`; `Nome` hint `Como ele aparece no menu e na lateral` (defaults to first word of command); `Desenho`: 10 icon chips 34×34 radius 7 (selected bg `bgActive` + accent border) | `Cancelar` / `Salvar` → `store.addLauncher` / `store.editLauncher` |

Launcher icons: `Terminal`, `Monitor`, `Gráfico`, `Git`, `Servidor`, `Banco`, `Foguete`,
`Bug`, `Editor`, `Caixa`. Launcher color = `groupTints[sum(codeUnits(id)) % 4]`.

#### Flow editor (`ui/flow.dart`)

Dialog (barrier `#66000000`), bg `bgSidebar`, inset 28, max 720×640.
Header (padding 20/16/16/14, bottom border): `account_tree_outlined` 16 purple · `Novo
fluxo` / `Fluxo` (14 w600) · subtitle 12 fgFaint `Em {where}` or `"{n} passo(s) depois de
"{title}""` (`passo`/`passos`) · close ×.
Timeline (`_Node`: 34 px gutter with 24 px circular bullet — `bgActive` or dim `bg`,
`border` outline — and a 1.5 px `border` connector line; content padded 12 below):
1. Trigger card (tinted `bgActive`, radius 9): new → `Abre uma sessão em {where}` + `Ela é
   o primeiro passo: nasce com o prompt abaixo e o resto do fluxo já pendurado nela.` +
   field `Como chamar o painel (opcional)` + field (3–8 lines, autofocus) `O que pedir pra
   ela — o prompt de abertura`; existing → `Quando essa sessão terminar` + status line
   (dot 7 in status color, `"{title} · {status}[ · {n} agente(s) rodando][ · {hold
   label}]"`).
2. One card per step, bullet = mono step number in accent: kind tiles (selected accent
   border), tools `Subir`/`Descer`/`Tirar do fluxo`, blurb, (handoff) target dropdown
   hint `Pra qual painel`, text field (command kind: 1 line mono).
3. Adder: `Por onde o fluxo começa?` / `E depois?` + add tiles (202 wide, hover accent).
Footer (top border): rule text (11 fgFaint) · `Cancelar` · primary: new → `Abrir a sessão`
(no steps) / `Abrir e armar`; existing → `Limpar a fila` (no steps) / `Disparar` (session at
rest) / `Armar`. Disabled for new flow until the opening prompt is filled.
Actions: new → `store.startFlow(folder, cwd, feature, prompt, label, steps)`; existing →
`store.queue(tab, steps, now: tab.status.atRest)`.
Rule texts:
- new: `A sessão abre com o prompt acima; o passo 1 sai quando ela terminar de verdade — turno encerrado, nenhum agente dela ainda rodando e alguns segundos de silêncio. A fila não sobrevive a fechar o app.`
- existing: `Um passo por vez: sai quando a sessão terminar de verdade — turno encerrado, nenhum agente dela ainda rodando e alguns segundos de silêncio. Passo que não devolve o turno pra ela (comando, sessão nova, passar a bola) já emenda no seguinte. A fila não sobrevive a fechar o app.`

Step kinds (handoff only offered when another live Claude tab exists in the same folder):

| kind | short | icon | blurb | hint |
|---|---|---|---|---|
| keepGoing | `Continuar` | `subdirectory_arrow_right` | `Manda o texto de volta pra esta mesma sessão` | `O que mandar pra ela em seguida` |
| newSession | `Outra sessão` | `add_comment_outlined` | `Abre uma sessão nova na mesma pasta, já com esse prompt` | `O prompt com que a sessão nova abre` |
| command | `Comando` | `terminal` | `Sobe um terminal rodando a linha abaixo` | `flutter analyze && flutter test` |
| handoff | `Passar a bola` | `swap_horiz` | `Entrega o recado final desta sessão a outro painel` | `O recado que vai junto com a última mensagem dela` |

A new `newSession` step is prefilled with:
`Revise o que acabou de ser feito nesta worktree: leia `git status` e `git diff`, procure bugs, regressões e coisas fora do padrão do repo. Não altere nada — só relate o que achou.`
Hold labels: `disparando`, `a sessão ainda está trabalhando`, `esperando os agentes que ela
abriu`, `confirmando que ela parou mesmo`.

### 1.9 Settings (`ui/settings.dart`)

Single-instance dialog (re-open is a no-op), barrier `#33000000`, bg `bgSidebar`, inset 28,
fixed 800 × 620, rebuilds live on theme/typography/store changes.
Left rail (184 wide, bg `bg`, right border): title `Configurações` (13 w600, padding
16/18/16/14) then sections (margin 8/1, padding 10/8, radius 7, selected bg `bgActive` +
icon accent + w600; hover `bgHover`; icon 15; label 12.5):

| Section | Icon | Opened by |
|---|---|---|
| `Aparência` | `palette_outlined` | default |
| `Atalhos` | `keyboard_outlined` | |
| `Programas` | `rocket_launch_outlined` | |
| `Plugins` | `extension_outlined` | plugin header gear |
| `Sessões` | `bedtime_outlined` | |
| `Conta & uso` | `speed_outlined` | footer account icon |

Body: scroll, padding 24/22/24/12. Footer (top border, padding 16/8): `Tudo aqui vale na
hora` (11 fgFaint) · `Pronto` (closes). Section heading: 14 w600 + hint 11.5/1.35 fgFaint
(or an ⓘ tooltip, max 360 wide, 200 ms, when `hintAsInfo`).

**Aparência**
- `Tema` — hint `Clicar já aplica — a janela atrás deste diálogo é o preview, e o pty vai junto: a paleta pinta o scrollback, não só a moldura.` Theme gallery (2.6). Click → `store.setTheme(p)`.
- `Tipografia do pty` — hint `Vale pra todo painel. {zoomIn key} e {zoomOut key} mexem em um só, a partir daqui — e o painel lembra o que você deixou.`; `Restaurar padrões` (only when ≠ standard) → `store.setTypography(MxType.standard)`. Sample block (terminal bg/fg, radius 8): `$ ` (brightBlack) `git` (green) ` status --short` / ` M ` (yellow) `lib/ui/settings.dart` / `?? Il1 O0 — {} => (i:M)` (brightBlack). Face chips (only installed faces; chip padding 10/7, radius 7, selected accent border; family name rendered in its own face 12 + note 10.5): → `setTypography(copyWith(mono))`. Note `Só as que existem nesta máquina; a Hack vem no app`. Steppers `Corpo` (`"{n}px"`, step 1, 9–24) and `Entrelinha` (`"{x.xx}"`, step 0.1, 1.0–1.8). Stepper: label 11 fgFaint; box bg `bg` radius 7 border; `−` (`Menos`) · value mono 12 in 58 px · `+` (`Mais`).
- `Barra de status` — hint `A faixa com o resumo das sessões e o sino. O sino, a lista dele e os cartões de aviso ficam do mesmo lado que ela.`; switch `Barra de status em cima` / `No topo da janela, em vez de no pé` → `store.setStatusBarTop`.

**Atalhos** — heading hint `Clique numa tecla pra trocar, no + pra dar uma segunda combinação à mesma ação. Uma tecla só tem um dono: dar a outra ação tira de quem a tinha.`; `Restaurar padrões` → `store.resetShortcuts()`. Grouped by `Sessões`, `Navegação`, `Exibição`, `Janela` (10.5 w600 ls 0.6). Row: label 12.5 + hint 11; keycaps (h 26, radius 6, mono 12; hover: accent border + remove × if >1 chord; tooltip `Trocar`) → record; `+` (`Adicionar outra combinação`) and `undo` (`Voltar ao padrão`, only if changed) → `store.resetShortcuts(action)`. Recorder: accent-bordered box `Pressione a combinação   ⎋ cancela` (11 accent); Esc cancels, modifiers alone ignored, rejected chord shows yellow `block` + reason. Bind → `store.bindShortcut(action, chord, replacing:)`; remove → `store.unbindShortcut`. Box `Fixos` lists: `⌘1 … ⌘9` `Ir pra enésima sessão da lateral`; paste chords `Colar no painel — imagem inclusa, que o Claude recebe como ^V`; (linux) copy `Copiar a seleção — o Ctrl+C é do processo`; `⇧↩` `Quebrar linha no prompt sem enviar`; `⌃C` `Do processo, como em qualquer terminal`; linux `Alt+F4` `Do desktop: fechar a janela` / mac `⌘Q ⌘W ⌘H ⌘M` `Do macOS: sair, fechar, esconder, minimizar`.

**Programas** — hint `Um painel que já abre dentro de um comando — é o que a sessão do Claude sempre foi, com o comando vindo daqui. Cada um vira uma linha no + da lateral, e volta rodando quando o app reabre.` Empty box: `Nenhum ainda. Um `btop` aqui é um painel de monitor a um clique de distância; um `npm run dev` é o servidor do projeto sempre no mesmo lugar.` Rows (radius 9, hover `bgHover`): icon 17 · name 12.5 w600 + command mono 11.5 · `1 painel aberto`/`{n} painéis abertos` · edit (`Editar`) · × (`Apagar` / `Apagar — os {n} painéis continuam abertos, como terminais`) → `store.removeLauncher`. Button `Novo programa…`.

**Plugins** — heading ⓘ `Temas, comandos e janelas que vêm de fora, instalados em ~/.maestria/plugins. Um plugin com programa roda com as suas permissões — instale só o que você confia. Como escrever um: docs/plugins.md, no repositório da maestria.` Buttons `Instalar…`, `Abrir a pasta`, `Reler` (`plugins.scan(); plugins.startup()`). Empty: `Nenhum ainda. Os exemplos do repositório (examples/plugins) são um bom começo: um tema com atalhos, e um plugin completo em node com comandos, hooks e uma janela.` Plugin card: glyph in state tone · name · version (mono) · state label (`Com problema`, `Desligado`, `Pronto`, `Subindo`, `Rodando`, `Parou`; tones red/fgFaint/fgDim/yellow/green/red) · `Desenvolvimento` (purple, linked). Details: description (1 line), `Comandos: …`, `Temas: {labels} — em aparência`, expandable `Ver mais`/`Ver menos` (purple). ⋮ menu (`Mais`; yellow + `Mais — atalho em conflito em configurar` when key clashes): `Reiniciar` (`Relê a pasta e sobe o processo de novo`), `Configurar…` (subtitle `1 atalho em conflito`/`{n} atalhos em conflito` yellow), `Log`, `Mostrar no Finder`, —, `Remover…` (red). Switch → `store.setPluginEnabled`. Footer link `Escrevendo um plugin? Carregar pasta de desenvolvimento…`.

**Sessões** — `Hibernação`, hint `Uma sessão do Claude parada no prompt custa uns 200MB de RAM pra não fazer nada. Hibernar é desligar o processo e ficar com a conversa: a linha continua na lateral, e o clique nela retoma de onde parou. Também dá pra hibernar uma sessão à mão, pelo menu da linha.` Switch `Hibernar sozinho o que está parado fora da tela` (subtitle `Só as que não estão na tela, sem pendência com você, sem fila pra andar e sem agentes rodando. O que estava rodando dentro dela — um servidor MCP, um dev server que uma ferramenta subiu — vai junto com o processo.`) → `store.setHibernateMinutes(on ? 30 : 0)`. Stepper `Depois de quanto tempo parada`: shows `"{h} h"` when a whole number of hours ≥ 60 min else `"{m} min"`; less: ≤5→5, ≤30→−5, ≤120→−15, else −60; more: <30→+5, <120→+15, else +60; bounds 5 min … 24 h.

**Conta & uso** — hint `O mesmo que o /usage do Claude mostra, lido da credencial desta máquina`. Loading skeleton (3 grey bars), error card (`cloud_off_outlined` + problem + reload), meters: org · plan (12.5 w600), email 11.5; per window: label 12.5, `"{p}%"` mono w600, 5 px bar (track = color@0.3), color accent <75, yellow ≥75, red ≥90; `Reseta em {shortUntil}`; credits line; `Lida às HH:MM` + reload (`Ler de novo`).

(Dictation settings exist but are commented out — feature excluded from this version.)

### 1.10 Default keymap (for the shortcuts page)

mac label format: `⌃⌥⇧⌘` + key glyph concatenated; Linux: `Ctrl+Shift+Alt+Super+Key`.

| id | label | mac default | linux default |
|---|---|---|---|
| claude | `Nova sessão Claude` | ⌘T | Ctrl+Shift+T |
| shell | `Novo shell` | ⇧⌘T | Ctrl+Shift+Enter |
| task | `Nova task` | ⌘N | Ctrl+Shift+N |
| rename | `Renomear o painel em foco` | ⌘E | F2 |
| pin | `Prender o painel em foco` | ⌥⌘P | Ctrl+Alt+P |
| close | `Fechar o painel em foco` | ⌘⌫ | Ctrl+Shift+W |
| close-settled | `Fechar as sessões resolvidas` | ⇧⌘K | Ctrl+Shift+K |
| next-pane | `Próximo painel` | ⌃Tab, ⌥⌘↓ | Ctrl+Tab, Ctrl+PgDn |
| prev-pane | `Painel anterior` | ⌃⇧Tab, ⌥⌘↑ | Ctrl+Shift+Tab, Ctrl+PgUp |
| next-session | `Próxima sessão` | ⌥⌘→ | Ctrl+Shift+→ |
| prev-session | `Sessão anterior` | ⌥⌘← | Ctrl+Shift+← |
| search | `Buscar na lateral` | ⌘F | Ctrl+Shift+F |
| markdown | `Abrir um markdown…` | ⌘O | Ctrl+Shift+O |
| plan | `Ver o plano da sessão em foco` | ⇧⌘L | Ctrl+Shift+L |
| refresh | `Reler o git` | ⌘R | Ctrl+Shift+R |
| zoom-in | `Aumentar o corpo do painel` | ⌘=, ⇧⌘= | Ctrl+=, Ctrl+Shift+=, Ctrl+Shift++ |
| zoom-out | `Diminuir o corpo do painel` | ⌘− | Ctrl+− |
| zoom-reset | `Voltar o painel ao corpo base` | ⌘0 | Ctrl+0 |
| sidebar | `Esconder ou mostrar a lateral` | ⌥⌘S | Ctrl+Shift+B |
| settings | `Configurações` | ⌘, , ⇧⌘P | Ctrl+, , Ctrl+Shift+P |

Session slots: `⌘1…⌘9` (mac) / `Alt+1…Alt+9` (linux). Hints for each action are in
`lib/services/shortcuts.dart` (quote from there).

### 1.11 Prototype screenshots

`docs/prototipo.png`, `docs/prototipo-split.png` are an **early prototype** (title
`maestria`, sidebar header "maestria hooks :49488", per-folder `+ terminal / + sessão /
+ task` buttons, purple status dot, Claude Code banner in a pane). The current UI (above)
replaced that header with search + gear, the footer, rail and status bar. Use them only for
overall proportions: ~300 px dark sidebar card on the left, pane on the right, terminal
text in the mono face.

---

## 2. Theme system (`lib/theme.dart`)

### 2.1 Palette tokens

| token | role |
|---|---|
| `canvas` | window background behind every card; gutters |
| `bg` | pane/card background; terminal background |
| `bgSidebar` | sidebar card, dialogs, popovers, pane header |
| `bgHover` | hover fill |
| `bgActive` | selected/active fill; menus background; tooltips |
| `border` | hairlines, card outlines, dividers |
| `fg` / `fgDim` / `fgFaint` | text: primary / secondary / tertiary & disabled |
| `accent` | focus, primary buttons, links, cursor, working status |
| `green` / `yellow` / `red` / `purple` | status: idle-ready / needs-you / permission-danger / starting; purple also = feature/hotfix |
| `ansi.{black…white}` + `bright*` | terminal 16 colours; bright falls back to normal when absent |
| `dark` (bool) | Material base brightness; shadow strength |

Derived:
- `shadow` = dark ? `#59000000` : `#14101828` (ARGB).
- Terminal theme: cursor = accent; selection = accent @ 0.3; fg = `fg`; bg = `bg`; ansi as
  given; search hit bg = `yellow`, current hit bg = `accent`, hit fg = `canvas`.
- `onPrimary` (text on accent) = accent luminance > 0.5 ? `canvas` : white.
- `groupTints` (pane groups, launcher colours) = `[ansi.cyan, ansi.magenta, ansi.blue,
  ansi.brightGreen]`; colour = `tints[sum(utf16 code units of id) % 4]`.
- `Mx.claude = #D97757` — the Claude mark colour, never themed.
- Status colours: working/tool → accent; waitingInput/waitingAnswer → yellow;
  waitingPermission → red; idle/ready → green; starting → purple; others → fgFaint.
  Labels: `—` unknown, `Abrindo`, `Pronto`, `Pensando`, `Rodando`, `Esperando você`,
  `Pergunta`, `Permissão`, `Ocioso`, `Encerrada`.

Global Material overrides: scaffold/canvas = canvas; divider = border; dialog bg
bgSidebar radius 12 border; tooltip bg bgActive radius 6 border, text 11 fg, **wait 400
ms** everywhere; popup menus as in 1.7 (labels 12, disabled fgFaint).

Geometry: gap 8, panel radius 10, menus radius 10, dialogs 12, popovers/cards 8, rows 9,
small buttons 4–6.

### 2.2 Built-in themes (picker order; hex `RRGGBB`, alpha FF)

| # | id | label | dark | canvas | bg | bgSidebar | bgHover | bgActive | border | fg | fgDim | fgFaint | accent | green | yellow | red | purple |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | `maestria` | Dark | true | 0D0F13 | 16181D | 1B1E24 | 23272F | 2C313A | 2A2F38 | E6E8EB | 8B929E | 5C636F | 7AA2F7 | 6ECF8F | E0AF68 | F07178 | BB9AF7 |
| 2 | `ayu-dark` | Ayu Dark | true | 07090F | 0D1017 | 0B0E14 | 191E29 | 242A36 | 1C222C | BFBDB6 | 8A8986 | 565B66 | E6B450 | AAD94C | FFB454 | D95757 | D2A6FF |
| 3 | `claude-dark` | Claude Dark | true | 262624 | 30302E | 2B2A28 | 3D3C38 | 4C4A45 | 3B3A35 | F5F4EF | C2BFB5 | 918D83 | C96442 | 7FA968 | D9A441 | E0575B | C08BB9 |
| 4 | `cobalt` | Cobalt | true | 122738 | 193549 | 15232D | 1F4662 | 245A7E | 234E6D | FFFFFF | 9FB8CF | 5B87AD | FFC600 | 3AD900 | FF9D00 | FF628C | FB94FF |
| 5 | `everforest-dark` | Everforest Dark | true | 232A2E | 2D353B | 272E33 | 3D484D | 475258 | 3D484D | D3C6AA | 9DA9A0 | 7A8478 | 83C092 | A7C080 | DBBC7F | E67E80 | D699B6 |
| 6 | `high-contrast` | High Contrast | true | 000000 | 0F0F0F | 080808 | 1E1E1E | 2E2E2E | 4A4A4A | FFFFFF | C6C6C6 | 8A8A8A | 2FAFFF | 44BC44 | D0BC00 | FF5F59 | B6A0FF |
| 7 | `night-owl` | Night Owl | true | 010E1A | 011627 | 011221 | 0B2942 | 1D3B53 | 122D42 | D6DEEB | 8FA4BE | 5F7E97 | 82AAFF | ADDB67 | ECC48D | EF5350 | C792EA |
| 8 | `gruvbox-dark` | Gruvbox Dark | true | 1D2021 | 282828 | 32302F | 3C3836 | 504945 | 3C3836 | EBDBB2 | BDAE93 | 928374 | 83A598 | B8BB26 | FABD2F | FB4934 | D3869B |
| 9 | `solarized-dark` | Solarized Dark | true | 00212B | 002B36 | 00252F | 073642 | 0E4B5A | 0A3A47 | 93A1A1 | 839496 | 586E75 | 268BD2 | 859900 | B58900 | DC322F | 6C71C4 |
| 10 | `graphite` | Graphite | true | 212121 | 303030 | 272727 | 3F3F3F | 515151 | 3D3D3D | ECECEC | B4B4B4 | 8F8F8F | 19C37D | 10A37F | E5A33A | E5484D | B87BF0 |
| 11 | `synthwave-84` | SynthWave '84 | true | 1A1727 | 262335 | 241B2F | 34294F | 463465 | 34294F | F0EFF1 | AFA9D3 | 848BBD | FF7EDB | 72F1B8 | FEDE5D | FE4450 | B893CE |
| 12 | `slate` | Slate | true | 1E2126 | 2A2E35 | 24282E | 353A43 | 414751 | 3A404A | E2E7EE | A0AAB8 | 6B7584 | 7FB2E5 | 7DC9A5 | D4BC7A | E27A88 | A6A0E0 |
| 13 | `monokai-pro` | Monokai Pro | true | 221F22 | 2D2A2E | 262327 | 3A373B | 4A474B | 403E41 | FCFCFA | C1C0C0 | 939293 | 78DCE8 | A9DC76 | FFD866 | FF6188 | AB9DF2 |
| 14 | `nord` | Nord | true | 242933 | 2E3440 | 2B303B | 3B4252 | 434C5E | 3B4252 | ECEFF4 | B7C0CE | 7B8797 | 88C0D0 | A3BE8C | EBCB8B | BF616A | B48EAD |
| 15 | `dracula` | Dracula | true | 1B1C24 | 282A36 | 21222C | 343746 | 44475A | 343746 | F8F8F2 | BFC7D5 | 6272A4 | BD93F9 | 50FA7B | F1FA8C | FF5555 | FF79C6 |
| 16 | `zenburn` | Zenburn | true | 2F2F2F | 3F3F3F | 383838 | 4F4F4F | 5F5F5F | 4A4A4A | DCDCCC | B8B8A6 | 909082 | 8CD0D3 | 60B48A | F0DFAF | CC9393 | DC8CC3 |
| 17 | `github-light` | GitHub Light | false | EAEEF2 | FFFFFF | F6F8FA | EFF2F5 | DDE3E9 | D8DEE4 | 1F2328 | 656D76 | 8C959F | 0969DA | 1A7F37 | 9A6700 | CF222E | 8250DF |
| 18 | `rose-pine-dawn` | Rosé Pine Dawn | false | EBE3DB | FFFAF3 | FAF4ED | F2E9E1 | DFDAD9 | E5DED6 | 575279 | 797593 | 9893A5 | 286983 | 56949F | EA9D34 | B4637A | 907AA9 |
| 19 | `gruvbox-light` | Gruvbox Light | false | EBDBB2 | FBF1C7 | F2E5BC | EBDBB2 | D5C4A1 | E1D2A9 | 3C3836 | 665C54 | 928374 | 076678 | 79740E | B57614 | 9D0006 | 8F3F71 |

ANSI (`=` = not set, falls back to the normal colour):

| id | black | red | green | yellow | blue | magenta | cyan | white | br.black | br.red | br.green | br.yellow | br.blue | br.magenta | br.cyan | br.white |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `maestria` | 16181D | F07178 | 6ECF8F | E0AF68 | 7AA2F7 | BB9AF7 | 7DCFFF | E6E8EB | 5C636F | FF8B92 | 86E0A4 | F0C589 | 94B4FF | CDB2FF | 96DBFF | FFFFFF |
| `ayu-dark` | 191E29 | EA6C73 | 91B362 | F9AF4F | 53BDFA | D2A6FF | 90E1C6 | C7C7C7 | 686868 | F07178 | C2D94C | FFB454 | 59C2FF | FFEE99 | 95E6CB | FFFFFF |
| `claude-dark` | 3D3C38 | E0575B | 7FA968 | D9A441 | 6F9BC4 | C08BB9 | 6FA8A0 | E8E5DC | 6E6A61 | EC7A72 | 9CC183 | EBC168 | 8FB5D9 | D3A5DA | 8CC2B9 | F5F4EF |
| `cobalt` | 1F4662 | FF628C | 3AD900 | FFC600 | 0088FF | FB94FF | 80FCFF | E0E8F0 | 5B87AD | FF2C6D | A5FF90 | FFE50A | 55AAFF | = | 9EFFFF | FFFFFF |
| `everforest-dark` | 343F44 | E67E80 | A7C080 | DBBC7F | 7FBBB3 | D699B6 | 83C092 | D3C6AA | 859289 | = | = | = | = | = | = | F2EFDF |
| `high-contrast` | 595959 | FF5F59 | 44BC44 | D0BC00 | 2FAFFF | FF66FF | 00D3D0 | E0E0E0 | 8A8A8A | FF8F88 | 70D73F | FEC43F | 79A8FF | F78FE7 | 6AE4B9 | FFFFFF |
| `night-owl` | 1D3B53 | EF5350 | 22DA6E | ADDB67 | 82AAFF | C792EA | 21C7A8 | D6DEEB | 5F7E97 | = | = | FFEB95 | = | = | 7FDBCA | FFFFFF |
| `gruvbox-dark` | 282828 | CC241D | 98971A | D79921 | 458588 | B16286 | 689D6A | A89984 | 928374 | FB4934 | B8BB26 | FABD2F | 83A598 | D3869B | 8EC07C | EBDBB2 |
| `solarized-dark` | 073642 | DC322F | 859900 | B58900 | 268BD2 | D33682 | 2AA198 | EEE8D5 | 586E75 | CB4B16 | 9FBF00 | D5A200 | 4FA8E0 | 8085D6 | 35BFB4 | FDF6E3 |
| `graphite` | 3D3D3D | E5484D | 10A37F | E5A33A | 4E9BF5 | B87BF0 | 2DBFB1 | D9D9D9 | 6E6E6E | FF6B6E | 19C37D | F5C15A | 74B3FF | CE9AFF | 55D8CB | ECECEC |
| `synthwave-84` | 34294F | FE4450 | 72F1B8 | FEDE5D | 8C8CFF | FF7EDB | 36F9F6 | F0EFF1 | 848BBD | F97E72 | = | F3E70F | A6A6FF | = | 03EDF9 | FFFFFF |
| `slate` | 353A43 | E27A88 | 7DC9A5 | D4BC7A | 7FB2E5 | A6A0E0 | 7CC7D6 | CBD2DC | 6B7584 | F0929E | 97DDBA | E6D094 | 9CC6F0 | BDB7EE | 98DAE7 | E2E7EE |
| `monokai-pro` | 403E41 | FF6188 | A9DC76 | FFD866 | FC9867 | AB9DF2 | 78DCE8 | FCFCFA | 727072 | = | = | = | = | = | = | FFFFFF |
| `nord` | 3B4252 | BF616A | A3BE8C | EBCB8B | 81A1C1 | B48EAD | 88C0D0 | E5E9F0 | 4C566A | = | = | = | = | = | 8FBCBB | ECEFF4 |
| `dracula` | 21222C | FF5555 | 50FA7B | F1FA8C | BD93F9 | FF79C6 | 8BE9FD | F8F8F2 | 6272A4 | FF6E6E | 69FF94 | FFFFA5 | D6ACFF | FF92DF | A4FFFF | FFFFFF |
| `zenburn` | 4D4D4D | 705050 | 60B48A | DFAF8F | 506070 | DC8CC3 | 8CD0D3 | DCDCCC | 709080 | DCA3A3 | C3BF9F | F0DFAF | 94BFF3 | EC93D3 | 93E0E3 | FFFFFF |
| `github-light` | 24292F | CF222E | 116329 | 7D4E00 | 0969DA | 8250DF | 1B7C83 | 6E7781 | 57606A | A40E26 | 1A7F37 | 9A6700 | 218BFF | A475F9 | 3192AA | 8C959F |
| `rose-pine-dawn` | F2E9E1 | B4637A | 286983 | EA9D34 | 56949F | 907AA9 | D7827E | 575279 | 9893A5 | = | = | = | = | = | = | 575279 |
| `gruvbox-light` | 665C54 | 9D0006 | 79740E | B57614 | 076678 | 8F3F71 | 427B58 | 7C6F64 | 928374 | CC241D | 98971A | D79921 | 458588 | B16286 | 689D6A | A89984 |

Theme resolution:
- Default theme `maestria`. `byId(id)` = built-in or plugin theme with that id ▸ retired
  alias ▸ default. Never fail on an unknown id.
- Retired aliases: `tokyo-night → maestria`, `catppuccin-frappe → catppuccin-mocha`,
  `rose-pine → catppuccin-mocha`, `chatgpt-dark → graphite` (catppuccin ids now come from
  the `maestria.catppuccin` plugin; without it they land on the default).
- `isBuiltIn(id)` = in the built-in list or a retired alias; plugins may not register a
  theme with such an id.
- Plugin themes live in a separate `extra` list (rebuilt on every plugin load), shown after
  the built-ins under `De plugins`.
- Persist the theme **id** only.

### 2.3 Theme JSON (plugin/user themes)

Declared in a plugin manifest as `"themes": [{ "path": "themes/meu.json" }]`. Format
(colours `#RRGGBB` or `#AARRGGBB`, leading `#` optional, whitespace trimmed):

```json
{
  "id": "meu-tema", "label": "Meu tema", "dark": true,
  "canvas": "#0B1016", "bg": "#121A22", "bgSidebar": "#16202A",
  "bgHover": "#1F2B37", "bgActive": "#283746", "border": "#223040",
  "fg": "#D8E3EC", "fgDim": "#9DB0C0", "fgFaint": "#5F7385",
  "accent": "#5EE3C1", "green": "#8BD88B", "yellow": "#F2CD73",
  "red": "#F07A83", "purple": "#B69CF5",
  "ansi": {
    "black": "#283746", "red": "#F07A83", "green": "#8BD88B", "yellow": "#F2CD73",
    "blue": "#72B6F2", "magenta": "#B69CF5", "cyan": "#5EE3C1", "white": "#D8E3EC",
    "brightBlack": "#5F7385"
  }
}
```

Validation (errors are shown to the theme author, Portuguese):
- `id` non-empty string else `"id" é obrigatório`; `label` likewise `"label" é obrigatório`;
  `ansi` object else `"ansi" é obrigatório`.
- every UI token and the 8 normal ansi colours required; bad/missing →
  `"<key>" precisa ser uma cor #RRGGBB` (key prefixed `ansi.` for ansi fields).
- `dark` defaults to true (only literal `false` makes it light).
- `bright*` optional; invalid values are silently treated as absent.

Plugin RFW panes receive `data.theme` as ints `0xAARRGGBB`: `canvas, bg, sidebar, hover,
active, border, fg, dim, faint, accent, onAccent, green, yellow, red, purple, blue, cyan,
magenta`, plus `dark` and `mono`.

### 2.4 Typography

- **One monospace face** for the pty and every mono label (pane header, branch names,
  chords, paths). Bundled: **Hack v3.003** (`family: Hack`, Regular/Bold 700/Italic/
  BoldItalic — real bold/italic faces are needed so a bold cell isn't wider than a cell).
- UI text face: platform default sans (Flutter Material default → Roboto-ish on Linux, SF
  on mac). Sizes used: 15 (dialog titles, pane title w700), 14.5 (sidebar row title),
  14 (settings headings), 13.5 (folder/workspace names w600), 13, 12.5 (most body), 12
  (menus, fields, subtitles), 11.5, 11, 10.5 (captions/mono meta), 10, 8.5 (rail badge).
- `MxType{mono, size, line}`; standard `Hack 13.0px / 1.2`. Size clamp 9–24 step 1; line
  clamp 1.0–1.8 step 0.1; values rounded to 2 decimals on input; unknown/uninstalled face
  → Hack. Persist only fields that differ from standard (`{"mono":…, "size":…,
  "line":…}`); `toString` = `"{mono} {size}px/{line}"`.
- Per-pane zoom stored as integer steps; `clampZoom(z) = clamp(z, ceil(9 − size),
  floor(24 − size))`; effective size = base + clamped zoom; changing the base moves every
  pane, keeping each pane's delta; a pane at zoom 0 persists no zoom; ⌘0 resets to 0.
- Cache one terminal style per (face, size, line) — identity matters for the terminal
  renderer (no re-measure on rebuild).
- Offered faces (shown only if installed — detection by measuring `iiiiiiii` vs `MMMMMMMM`
  at 40 px: equal widths ⇒ monospace resolved; bundled Hack always shown): `Hack` (note
  `vem no app`), `Hack Nerd Font` (`com os glifos do prompt`), `JetBrainsMono Nerd Font`,
  `FiraCode Nerd Font`, `MesloLGS NF`, `SF Mono` (`a do Terminal.app`), `Menlo` (`a do
  macOS`), `Monaco`, `JetBrains Mono`, `Fira Code`, `IBM Plex Mono`, `Source Code Pro`,
  `Roboto Mono`, `Inconsolata`, `Cascadia Code`, `Cascadia Mono`, `Iosevka`, `Berkeley
  Mono`, `Geist Mono`, `Commit Mono`, `Maple Mono`, `Monaspace Neon`, `Victor Mono`,
  `Anonymous Pro`, `DejaVu Sans Mono`, `Liberation Mono`, `Noto Sans Mono`, `Ubuntu Mono`,
  `Andale Mono`, `PT Mono`, `Courier New`. (In Rust: query fontconfig / CoreText instead of
  measuring.)
- Pty glyph fallback chain (Hack lacks `⏺ ✽ ⎿ ⏱ ☒`, Claude Code's markers): `Menlo,
  Monaco, STIX Two Math, Apple Symbols, Noto Sans Symbols 2, Noto Sans Symbols, DejaVu
  Sans, Noto Color Emoji, Liberation Mono, Courier New, monospace, sans-serif`. Selected
  face always first.

### 2.5 Label colours ("cores de etiqueta", `MxTint`) — 13 tints

Optional colour a user assigns to a pane, folder, workspace or feature/hotfix. Persisted by
**name** (enum name), resolved against the current palette at draw time. Unknown name →
no colour.

| name | label | OKLCH hue° |
|---|---|---|
| red | `Vermelho` | 25 |
| orange | `Laranja` | 55 |
| yellow | `Amarelo` | 95 |
| lime | `Lima` | 125 |
| green | `Verde` | 150 |
| aqua | `Verde-água` | 175 |
| cyan | `Ciano` | 205 |
| petrol | `Azul-petróleo` | 235 |
| violet | `Violeta` | 290 |
| magenta | `Magenta` | 330 |
| pink | `Rosa` | 0 |
| brown | `Marrom` | 55 |
| grey | `Cinza` | — |

Formula (per palette, cache per palette):
1. Base set = `[ansi.red, ansi.yellow, ansi.brightGreen(→green), ansi.cyan, ansi.magenta]`.
2. Convert each to OKLCH (Björn Ottosson matrices; sRGB → linear `v ≤ 0.04045 ? v/12.92 :
   ((v+0.055)/1.055)^2.4`):
   - `l = cbrt(0.4122214708 r + 0.5363325363 g + 0.0514459929 b)`
   - `m = cbrt(0.2119034982 r + 0.6806995451 g + 0.1073969566 b)`
   - `s = cbrt(0.0883024619 r + 0.2817188376 g + 0.6299787005 b)`
   - `L = 0.2104542553 l + 0.7936177850 m − 0.0040720468 s`
   - `a = 1.9779984951 l − 2.4285922050 m + 0.4505937099 s`
   - `b = 0.0259040371 l + 0.7827717662 m − 0.8086757660 s`; `C = √(a²+b²)`.
3. `L̄`, `C̄` = arithmetic means of the five.
4. Tint = OKLCH(`L̄`, `C̄`, hue) → sRGB; **brown** = OKLCH(`0.8·L̄`, `0.6·C̄`, 55);
   **grey** = `ansi.brightBlack` (falls back to `ansi.black`).
   Inverse: `a = C cos h, b = C sin h`; `l = (L + 0.3963377774a + 0.2158037573b)³`,
   `m = (L − 0.1055613458a − 0.0638541728b)³`, `s = (L − 0.0894841775a − 1.2914855480b)³`;
   `R = 4.0767416621l − 3.3077115913m + 0.2309699292s`,
   `G = −1.2684380046l + 2.6097574011m − 0.3413193965s`,
   `B = −0.0041960771l − 0.7034186147m + 1.7076069010s`; clamp each to [0,1], encode
   `x ≤ 0.0031308 ? 12.92x : 1.055x^(1/2.4) − 0.055`; ×255 rounded.

Reference results (computed with the formula above; allow ±1 per channel rounding):

| id | red | orange | yellow | lime | green | aqua | cyan | petrol | violet | magenta | pink | brown | grey |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `maestria` | FD968E | F4A065 | D0B650 | A5C569 | 79CD8C | 49D1B2 | 2ECDDC | 5BC3FB | B6A9FF | E499DD | F794B3 | AA7B5B | 5C636F |
| `ayu-dark` | FF9890 | FFA362 | D8BB47 | A9CC65 | 77D58D | 3CD9B7 | 00D5E6 | 54CAFF | BCADFF | ED9BE6 | FF96B9 | B27E5A | 686868 |
| `claude-dark` | DD867F | D68E5D | B7A14B | 92AD5F | 6DB57D | 47B89E | 34B5C2 | 55ACDC | A095E1 | C888C2 | D9849F | 956D51 | 6E6A61 |
| `cobalt` | FF9C93 | FFAB55 | EDC925 | B3DD5A | 70E990 | 00EDC5 | 00E9FE | 37DBFF | CBB7FF | FFA1FE | FF9AC7 | C68559 | 5B87AD |
| `everforest-dark` | E79992 | E0A075 | C4B068 | A3BC78 | 84C290 | 68C5AD | 5CC2CE | 71BBE5 | AFA7EA | D49BCE | E397AE | 9E7A61 | 859289 |
| `high-contrast` | FF7871 | FF8A00 | D8AD00 | 96C502 | 36D26C | 00D7AB | 00D1EB | 00C1FF | B298FF | F37FEA | FF75AD | B76F39 | 8A8A8A |
| `night-owl` | FF8078 | FB8E36 | CFAB00 | 97BF3A | 52CA74 | 00CFA8 | 00CADF | 00BCFF | AE9AFF | E785DF | FF7DAA | AD7044 | 5F7E97 |
| `gruvbox-dark` | DA6A64 | D1762D | AC8E00 | 7D9F31 | 44A860 | 00AC8C | 00A8BA | 009DDA | 9180E1 | C06EBA | D4688D | 905C38 | 928374 |
| `solarized-dark` | E15E59 | D76C00 | AE8A00 | 779E00 | 25A954 | 00AD88 | 00A8BD | 009AE3 | 8F78EA | C464BD | DB5B8A | 93572A | 586E75 |
| `graphite` | F3736D | E9812B | BF9D00 | 8AB030 | 47BB69 | 00BF9B | 00BACF | 00ADF4 | A08CFB | D678CF | ED719C | A1663C | 6E6E6E |
| `synthwave-84` | FF948B | FFA34A | E6C100 | ABD64E | 66E287 | 00E6BE | 00E2F8 | 20D3FF | C4AFFF | FF99F8 | FF91C0 | C18052 | 848BBD |
| `slate` | EB9D96 | E4A57A | C8B56D | A7C07C | 89C695 | 6EC9B1 | 62C7D2 | 76BFE9 | B4ABEE | D89FD2 | E89CB2 | A17D65 | 6B7584 |
| `monokai-pro` | FF9991 | FFA562 | DBBD47 | AACE66 | 78D88E | 3ADBBA | 00D7E9 | 54CCFF | BEAEFF | F09DE9 | FF97BB | B47F5B | 727072 |
| `nord` | DA9791 | D39D79 | BBAA6E | 9FB47B | 85BA8F | 70BCA8 | 67BAC3 | 76B3D7 | A9A2DB | C998C3 | D695A8 | 957662 | 4C566A |
| `dracula` | FF9B92 | FFA956 | EAC628 | B1DA5A | 70E68E | 00EAC2 | 00E5FA | 3AD8FF | C8B4FF | FF9FFA | FF98C4 | C38458 | 6272A4 |
| `zenburn` | CD958F | C89A7C | B2A573 | 9BAD7D | 86B28E | 75B4A2 | 6FB2BA | 7AADCB | A49ECE | BE96BA | CA93A3 | 8D7362 | 709080 |
| `github-light` | B1403D | A94E00 | 866800 | 587900 | 008239 | 008566 | 008193 | 0075B3 | 6C58B9 | 994693 | AB3F68 | 723F17 | 57606A |
| `rose-pine-dawn` | C0706A | B9784A | 9D8939 | 7B944D | 599B68 | 349E86 | 1E9BA7 | 4293BE | 897FC3 | AC72A7 | BC6E87 | 805B42 | 9893A5 |
| `gruvbox-light` | B04D49 | A8580F | 896E00 | 5F7D14 | 288545 | 00886C | 008594 | 007AB1 | 7161B6 | 9A5194 | AB4C6D | 724525 | 928374 |

Tint precedence for a pane row / pane card (`chosenTintOf`): feature/hotfix tint ▸ pane
tint ▸ folder tint ▸ tint of the first workspace (sidebar order) containing the folder.
Effective colour = chosen tint ▸ pane-group colour (`groupTints`). A folder header glyph
uses folder tint ▸ workspace tint.
Usage: sidebar row background blend (1.6), `_Nest` rail line, pane card 2 px border and
header `alphaBlend(tint@0.34, bgSidebar)` with a 3 px bottom border in the tint.

### 2.6 Theme gallery (settings)

`Wrap` spacing 12. Swatch 176 wide: preview box h 104, padding 6, bg = theme canvas,
radius 8, border = selected ? current accent 2 px : hover ? fgFaint : border (120 ms).
Mini window inside (all in the *previewed* theme): left card 52 wide (bgSidebar, 0.5 px
border, radius 3) with 3 px bars `fg 26`, `fgDim 34`, `accent 22`, `fgFaint 30` (4 px
apart); right card (bg) with dot 4 green + bar `ansi.blue 30`, bars `fg 44`,
`ansi.magenta 24`, `yellow 34`, `fgFaint 40`; bottom strip h 10 (bgSidebar) with dot 3
green, 2 px bar fgFaint 18, spacer, dot 3 red. Label row: check 13 accent (selected) ·
label 11.5 (fg w600 selected / fgDim) · `light_mode_outlined` 12 for light themes.

---

## 3. Icons and assets

| Path | What | Notes |
|---|---|---|
| `assets/fonts/Hack-Regular.ttf` | Hack 3.003 | MIT / Bitstream Vera; bundle in binary (`include_bytes!`) |
| `assets/fonts/Hack-Bold.ttf` | weight 700 | |
| `assets/fonts/Hack-Italic.ttf` | italic | |
| `assets/fonts/Hack-BoldItalic.ttf` | 700 italic | |
| `assets/fonts/README.md` | provenance | |
| `assets/icons/claude.svg` | Claude mark, 24×24 viewBox, single path | Simple Icons, CC0. Painted `#D97757` (`Mx.claude`) |
| `assets/icons/terminal.svg` | square filled screen (rx 7 at 1,1,22,22) with `>_` **cut out by a mask** (stroke 2.1, round caps) | own drawing; tinted by srcIn colour |
| `assets/icons/README.md` | icon rules: monochrome 24×24, no baked colour, cut-outs must be real transparency | |
| `macos/Runner/Assets.xcassets/AppIcon.appiconset/app_icon_{16,32,64,128,256,512,1024}.png` | app icon, RGBA squares | also used for Linux hicolor 32–512 |

All SVG icons are rendered monochrome with a colour filter (src-in). Everything else is
**Material Icons** (Flutter `Icons.*`, outlined/rounded variants as named in this doc).
For Rust: bundle the Material Symbols/Icons font (Apache-2.0) or a curated SVG subset; the
names used are: `space_dashboard_outlined, add, settings_outlined, create_new_folder_outlined,
history_outlined, speed_outlined, view_sidebar_outlined, search, close_rounded,
filter_list_rounded, filter_list_off_rounded, chevron_right, expand_more,
hexagon_outlined, more_horiz, more_vert, folder_outlined, folder_rounded, call_split,
link_off, link, open_in_new, drive_file_rename_outline, folder_off_outlined, track_changes,
bolt, sticky_note_2_outlined, task_alt, bedtime_outlined, circle, grid_view_rounded,
grid_off, clear_all, article_outlined, tune, terminal, account_tree_outlined,
description_outlined, add_circle_outline, apps_outlined, content_copy, content_paste,
circle_outlined, check_circle(_outline), refresh, play_arrow, checklist_rtl,
chat_bubble_outline, push_pin(_outlined), check_circle_outline, extension_outlined, close,
folder_open, delete_outline, home_outlined, cleaning_services_outlined, code,
assignment_outlined, remove_circle_outline, inbox_outlined, check_box(_rounded),
check_box_outline_blank(_rounded), unfold_more, info_outline, notifications_*,
done_all_rounded, clear_all_rounded, expand_more_rounded, question_mark_rounded,
lock_rounded, check_rounded, lightbulb_outline_rounded, chevron_right_rounded,
priority_high_rounded, palette_outlined, keyboard_outlined, rocket_launch_outlined,
edit_outlined, undo, remove, block, cloud_off_outlined, light_mode_outlined, check,
keyboard_arrow_up/down, arrow_right_alt, subdirectory_arrow_right, add_comment_outlined,
swap_horiz, monitor_heart_outlined, bar_chart, dns_outlined, storage_outlined,
bug_report_outlined, inventory_2_outlined, warning_amber_rounded, help_outline, replay,
logout, file_open_outlined, receipt_long_outlined, restart_alt, light_mode_outlined`.

### 3.1 ClaudeAvatar (status mark)

Claude mark (size S, `#D97757`) + status badge at bottom-right, badge size
`clamp(S·0.56, 9, 14)`, offset `−0.18·badge` right/bottom; whole avatar opacity 0.72 when
`dim`. Badges (circle filled `canvas`, 1.1 px ring in the status colour, icon scaled):

| status | badge |
|---|---|
| working, tool | spinner: faint full ring (color@0.22, 1.1 px) + 1.05π arc (1.3 px, round cap) rotating, period **950 ms** linear |
| starting | pulse: ring alpha 0.20+0.30t, center dot radius r·(0.26+0.16t) alpha 0.55+0.45t; t = easeInOut, **1100 ms**, reverse-repeat; colour purple |
| ready | `chevron_right_rounded` scale 0.94, green |
| idle | `check_rounded` scale 0.72, green |
| waitingInput | `priority_high_rounded`, yellow |
| waitingAnswer | `question_mark_rounded` scale 0.62, yellow |
| waitingPermission | `lock_rounded`, red |
| ended | `close_rounded`, fgFaint |
| unknown | none |

---

## 4. Animations

Respect "disable animations" (reduced motion) for notices: durations become 0.

### 4.1 Confetti (`ui/confetti.dart`)

Overlay on top of the whole window, **does not intercept pointer events**, removes itself
when done; nothing persisted. Two variants:

| | `fire` (feature/hotfix completed) | `puff` (pane marked done, at click position) |
|---|---|---|
| origin | both bottom corners: left (0,1) and right (1,1), in fractions of the overlay | the click point, as a fraction of overlay size |
| count | 55 per cannon (110 total) | 40 |
| life | 2600 ms | 1400 ms |
| fade starts at progress | 0.72 | 0.62 |
| angle | left: U(0.19,0.44)·π above horizontal (≈35°–80°); right: π − that | U(0, 2π) |
| speed (screen heights/s) | U(2.1, 3.1) | U(0.6, 1.5) |
| spin (rad/s) | U(−9, 9) | U(−11, 11) |
| flutter (rad/s) | U(5, 11) | U(6, 13) |
| phase | U(0, 2π) | U(0, 2π) |
| width × height px | U(5,9) × U(3,12) | U(4,7) × U(3,8) |
| delay s | U(0, 0.16) | 0 |
| colour index | randInt(7) | randInt(7) |

Colours (current palette, by index): `accent, green, yellow, purple, red, ansi.cyan,
ansi.blue`. Physics (closed form, stateless per frame; unit = overlay height H):
`g = 1.5`, `drag k = 1.15`; t = seconds − delay (skip if ≤0);
`decay = (1 − e^(−k t))/k`; `vt = g·H/k`; `vx = cos(angle)·speed·H`; `vy =
−sin(angle)·speed·H`; `x = ox·W + vx·decay`; `y = oy·H + (vy − vt)·decay + vt·t`; skip if
`y > H + 40`. Draw: translate(x,y), rotate(spin·t), scale X by `cos(phase + flutter·t)`,
filled rect w×h centred. Alpha = 1 until fadeFrom, then `1 − (p − fadeFrom)/(1 − fadeFrom)`
where p = elapsed/life.

### 4.2 Reveal

`store.revealTab`: highlight lasts **2 s** (timer; cleared early only by another reveal),
row overlay = accent@0.12 fill + 1.5 px accent border, radius 6; scroll-into-view once,
alignment 0.3, **250 ms easeOut**. Triggered by the pane header button (tooltip `mostrar na
lateral`).

### 4.3 Flow (sidebar "brood" + editor)

No timed animation: the flow editor is a dialog (1.8). In the sidebar, panes spawned by a
flow step (`bornOf`) render nested one level under their parent **only while contiguous**
after it; an unrelated row in between breaks the nesting.

### 4.4 Notices / toasts / popovers

- Popovers (session list, notice center) and banner: 160 ms easeOut, opacity 0→1 +
  translate 8 px → 0 (from the bar side).
- Toast in: controller 280 ms; size grows over interval [0, 0.45] easeOut; slide from
  x = +1.15 widths and fade over [0.25, 1] easeOutCubic; out: reverse with easeInCubic,
  exits to the same side (right), then removed.
- Notice row dismiss in list: 280 ms: slide right 1 width + fade over [0, 0.7] easeInCubic,
  collapse height over [0.55, 1] easeInOut, then `store.dismissNotice`.
- "Limpar tudo" cascade: rows leave one by one with 35 ms stagger (cap 8 steps), each 280
  ms slide+fade (easeInCubic); then 220 ms settle: list height collapses (easeInOutCubic)
  while the empty text grows/fades in (fade over the last 60%). Notices are removed from the
  store only at the end; closing the list mid-animation still removes them.
- Hover reveals: `+` / clear buttons fade 110 ms easeOut. Sidebar grip handle 120 ms.
  Theme swatch border 120 ms. Flow add tiles 90 ms.
- Tooltips: 400 ms wait (settings ⓘ 200 ms; close-pane 600 ms; keycap "Trocar" 600 ms).

---

## 5. Packaging

### 5.1 Versioning

`pubspec.yaml` `version: X.Y.Z+R` → Debian version `X.Y.Z-R` (R defaults 1); file names
`maestria_X.Y.Z-R_amd64.deb`, `maestria_X.Y.Z-R_macos.dmg`; git tag `vX.Y.Z` (no
revision). In Rust: `Cargo.toml` `version = "X.Y.Z"` + `[package.metadata.deb] revision =
"R"`. `make bump V=…` rewrites the version (apt only upgrades when the version changes).

### 5.2 Linux `.deb` (current layout)

Built in an `ubuntu:22.04` container (glibc 2.35 floor) by
`packaging/linux/make-deb.sh`:

```
/opt/maestria/maestria                 (bundle: binary + lib/*.so + data/)
/usr/bin/maestria -> /opt/maestria/maestria
/usr/share/applications/br.com.marrow.maestria.desktop
/usr/share/icons/hicolor/{32,64,128,256,512}x…/apps/br.com.marrow.maestria.png
DEBIAN/control, DEBIAN/postinst, DEBIAN/postrm (identical)
```

Desktop file (verbatim):

```
[Desktop Entry]
Type=Application
Name=MaestrIA
Comment=Cockpit para sessões do Claude Code
Exec=/opt/maestria/maestria
Icon=br.com.marrow.maestria
Terminal=false
Categories=Development;
StartupWMClass=br.com.marrow.maestria
```

control:

```
Package: maestria
Version: X.Y.Z-R
Section: devel
Priority: optional
Architecture: amd64
Depends: <dpkg-shlibdeps result, fallback "libgtk-3-0 (>= 3.24), libglib2.0-0">, libegl1, libgles2, git, libnotify-bin, xdg-utils, curl
Recommends: dbus-x11
Maintainer: Matheus Pano <matheus@marrow.com.br>
Description: Cockpit para sessões do Claude Code
 Painéis lado a lado, cada um com uma sessão do Claude Code viva dentro,
 worktrees do git por tarefa e um leitor de markdown para o que as sessões
 escrevem. Requer o Claude Code CLI (claude) instalado e no PATH.
```

postinst/postrm: `gtk-update-icon-cache -q -t -f /usr/share/icons/hicolor || true`;
`update-desktop-database -q /usr/share/applications || true`.
Runtime deps rationale: `git` (worktrees), `libnotify-bin` (`notify-send`), `xdg-utils`
(`xdg-open`), `curl`; libs that are `dlopen`ed (EGL/GLES for Flutter) must be listed by
hand because shlibdeps cannot see them. `claude` is not a dependency (user-installed).

**cargo-deb equivalent** (Rust binary is self-contained; keep the same visible paths so
upgrades from the Flutter package are seamless):

```toml
[package.metadata.deb]
name = "maestria"
maintainer = "Matheus Pano <matheus@marrow.com.br>"
section = "devel"
priority = "optional"
revision = "1"
extended-description = """Painéis lado a lado, cada um com uma sessão do Claude Code viva dentro,
worktrees do git por tarefa e um leitor de markdown para o que as sessões
escrevem. Requer o Claude Code CLI (claude) instalado e no PATH."""
depends = "$auto, git, libnotify-bin, xdg-utils, curl"   # + dlopen'd GPU libs, e.g. libvulkan1 / libegl1 / libxkbcommon0 depending on the GUI stack
recommends = "dbus-x11"
assets = [
  ["target/release/maestria", "opt/maestria/maestria", "755"],
  ["packaging/linux/br.com.marrow.maestria.desktop", "usr/share/applications/", "644"],
  ["assets/appicon/32.png",  "usr/share/icons/hicolor/32x32/apps/br.com.marrow.maestria.png", "644"],
  # … 64, 128, 256, 512
]
maintainer-scripts = "packaging/linux/debian/"   # postinst/postrm as above, plus the /usr/bin symlink (or ship the binary at usr/bin and set Exec accordingly)
```

The window must set the Wayland `app_id` and X11 `WM_CLASS` to `br.com.marrow.maestria`
(otherwise GNOME shows a second generic dock icon). Build in `ubuntu:22.04` (or use
`cargo-zigbuild --target x86_64-unknown-linux-gnu.2.35`) to keep the glibc floor. Keep the
smoke test: install the `.deb` in a clean `ubuntu:22.04` with apt, add `xvfb dbus-x11
libgl1-mesa-dri`, run `xvfb-run -a timeout 20 maestria`; exit code 124 (still alive after
20 s) = pass.

### 5.3 macOS `.dmg`

`packaging/macos/make-dmg.sh`:
1. `flutter build macos --release` → `build/macos/Build/Products/Release/maestria.app`.
2. Verify the binary is **universal** (`lipo -info` contains both `x86_64` and `arm64`),
   else fail.
3. Stage with `ditto` (preserves the ad-hoc signature), add symlink `Applications ->
   /Applications`, `codesign --verify --deep` must pass.
4. `hdiutil create -volname "maestria X.Y.Z" -srcfolder <stage> -ov -format UDZO
   packaging/macos/dist/maestria_X.Y.Z-R_macos.dmg`.

Bundle: `maestria.app`, `CFBundleName`/`CFBundleDisplayName` = `MaestrIA`,
`CFBundleIdentifier` = `br.com.marrow.maestria`, `CFBundleShortVersionString` = X.Y.Z,
`CFBundleVersion` = R, `LSMinimumSystemVersion` = 12.0, `NSPrincipalClass` NSApplication,
copyright `Copyright © 2026 br.com.marrow. All rights reserved.`. Signed **ad-hoc** only
(no notarization) — users run `xattr -dr com.apple.quarantine /Applications/maestria.app`
once. Entitlements (release): `com.apple.security.network.server`, `…network.client` (the
local hook HTTP server); no `app-sandbox` key in the entitlements (the app is not
effectively sandboxed — it must spawn shells and read `~`). Microphone entitlement/usage
string are commented out (dictation excluded).

Rust: `cargo build --release --target aarch64-apple-darwin` and `x86_64-apple-darwin`,
`lipo -create` into the bundle; `cargo-bundle` (or `cargo-packager`) with
`identifier = "br.com.marrow.maestria"`, `name = "MaestrIA"`, `osx_minimum_system_version
= "12.0"`, icon set from the PNGs; then `codesign --force --deep -s - maestria.app` and the
same `hdiutil` step. Keep the bundle file name `maestria.app` (docs tell users that path).

### 5.4 CI release (`.github/workflows/release.yml`)

Trigger: push tag `v*`, or `workflow_dispatch` with input `tag` (re-package an existing
tag). `permissions: contents: write`. Jobs:
1. `versao` (ubuntu-latest): checkout the tag; fail with
   `::error::a tag é $TAG e o pubspec diz $V -- rode 'make bump V=...' antes de taguear.`
   if `TAG != v<version>`.
2. `deb` (ubuntu-latest, needs versao): `./packaging/linux/build-deb.sh` (Docker, native
   amd64) → upload artifact `deb` (`packaging/linux/dist/*.deb`, error if none).
3. `dmg` (macos-14 arm64, needs versao): install pinned toolchain (cached), build,
   `./packaging/macos/make-dmg.sh` → artifact `dmg`.
4. `publicar` (needs deb, dmg): download artifacts merged into `anexos/`; if release exists
   `gh release upload $TAG anexos/* --clobber`, else `gh release create $TAG anexos/*
   --title "maestria ${TAG#v}" --notes-file packaging/NOTAS-DA-RELEASE.md`.

Local `make` targets to mirror: `help` (default), `deb`, `deb-only`, `deb-test`, `dmg`,
`release` (analyze+test+deb+smoke), `tag` (refuse with uncommitted changes; tag and push
`vX.Y.Z`), `analyze`, `test`, `bump V=`, `clean`. Release page:
`https://github.com/MatheusPano/MaestrIA/releases/tag/vX.Y.Z`. Toolchain version pinned in
one place (Makefile) and reused by Docker build-arg and CI.

---

## 6. UI test cases (language-neutral)

| # | Given | When | Then |
|---|---|---|---|
| T1 | built-in themes | list ids | ids unique; ≥3 light themes; no two share (canvas,bg,bgSidebar) |
| T2 | dark built-ins | compare bg luminance | max/min > 5; at least one dark bg has r==g==b |
| T3 | ids `nord`, `theme-from-the-future`, null | `byId` | `nord`; default; default |
| T4 | retired ids | `byId('tokyo-night')`, `byId('chatgpt-dark')` | `maestria`, `graphite` |
| T5 | no catppuccin plugin | `byId('catppuccin-mocha'/'catppuccin-frappe')`; `isBuiltIn('catppuccin-mocha')` | default; false |
| T6 | a plugin theme registered with id `catppuccin-mocha` | `byId` of `catppuccin-mocha`, `catppuccin-frappe`, `rose-pine` | all return the plugin theme |
| T7 | every built-in | inspect | accent ≠ yellow and accent ≠ purple |
| T8 | nord / github-light | terminal theme | bg = palette bg; fg = palette fg |
| T9 | tints | list labels | the 13 labels in order (Vermelho … Cinza) |
| T10 | every built-in | CIE76 ΔE between any two tints | ≥ 10 |
| T11 | every built-in | ΔE(tint, palette.bg) | ≥ 15 |
| T12 | nord then github-light then nord | read `cyan` tint | differs across themes; same value when back on nord |
| T13 | folder with each tint | serialize/deserialize | same tint |
| T14 | sidebar shown in default theme, empty state | switch to nord | empty-state text colour becomes nord.fgFaint without restart |
| T15 | light theme applied | inspect | widget brightness light; scaffold bg = canvas |
| T16 | typography | defaults | Hack 13 / 1.2 |
| T17 | config `{size: 400, line: 0.2}` | load | clamped to 24 / 1.0 |
| T18 | config mono = uninstalled face | load | Hack |
| T19 | standard typography | serialize | `{}`; non-standard round-trips equal |
| T20 | line stepped +0.1 ×3 from 1.0 | read | exactly 1.3 (rounded to 2 decimals) |
| T21 | base 13 | zoom +20 steps, then −1 | effective 24, then 23 (one ⌘− moves one point; stored zoom is clamped). Base 24: clampZoom(4)=0; base 9: clampZoom(−4)=0, sizeAt(2)=11 |
| T22 | base 13, pane zoom +2 (15) | base changed to 10 | pane size 12 (delta kept) |
| T22b | pane zoom 6 persisted | base becomes 23 | restored zoom clamped to 1; pane at base persists no `zoom` key |
| T23 | zoom reset key | press | pane zoom 0; `⌘0` is not one of the 9 slot chords |
| T24 | same (face,size,line) | request style twice | same instance (no re-measure) |
| T25 | settings opened at 1200×820 | appearance section | shows `Tipografia do pty`, `Corpo`, `13px`, `1.20`, `Hack`; tapping `Mais` shows `14px` and base becomes 14 |
| T26 | feature menu | open | order: `Sessão do Claude` < `Terminal` < `Montar um fluxo…` < `Renomear`; no `retomar conversa…` |
| T27 | folder `+` menu | pick `Sessão do Claude` | Claude opened in folder root, no label |
| T28 | feature menu | pick `Renomear` | dialog `Renomear feature`; no pane opened |
| T29 | folder with a launcher, worktrees | right-click folder | shows `Sessão do Claude`, `Terminal`, `Meus programas`, `Worktrees`, `Remover pasta` |
| T30 | folder menu, picker returns `/repo/docs/NOTAS.md` | `Abrir um markdown…` | picker starts in `/repo`; reader pane opened for that file, cwd `/repo` |
| T31 | worktree menu | `Abrir um markdown…` | picker starts in worktree path |
| T32 | panel menu of a Claude tab | open | no `Abrir um markdown…`; has `Renomear…`, `Ver o plano` |
| T33 | feature menu | pick `Terminal` | shell attached to the feature |
| T34 | worktree `TASK-47730` | `Sessão do Claude` / `Terminal` | cwd = worktree path, label `TASK#47730`; main checkout gets no label |
| T35 | worktree menu | `Copiar caminho` | clipboard = path; banner `Caminho copiado`; nothing opened |
| T36 | panel without tint | open menu | row `Cor`; hover → pick `Magenta` → tab tint magenta; reopen shows `Cor: Magenta`; `Sem cor` clears |
| T37 | window 800×240 | open `Cor` submenu | no overflow; submenu within [8, 232]; scrolls |
| T38 | feature tinted red | inspect tab of the feature | tab tint unset but effective colour red; panel menu shows disabled `Cor: … — da feature` |
| T39 | folder tint chosen | inspect | repo glyph and its tabs take the tint |
| T40 | launcher submenu | hover/click `Meus programas` then a launcher | launcher pane opens; submenu closes with its menu |
| T41 | sidebar visible | click `view_sidebar_outlined` twice | hidden then shown |
| T42 | width 420 | hide | width kept 420; `sidebarHidden: true` persisted; key absent when shown again |
| T43 | sidebar hidden | press search shortcut | sidebar shown |
| T44 | folder mirrored in two workspaces with a tab | render | both workspaces show it; `2 pastas`/`1 pasta`; collapsing in one leaves the other expanded |
| T45 | empty workspace | render | `0 pastas` and `Arraste uma pasta pra cá` |
| T46 | workspace tinted red, folder untinted | render | RepoGlyph colour = red tint |
| T47 | drag folder to middle of workspace header | drop | folder appended to workspace |
| T48 | drag folder to header bottom edge | drop | only reorders root rows |
| T49 | drag workspace folder onto a root row | drop | folder leaves workspace, stands alone |
| T50 | drag workspace onto middle of another workspace header | drop | only reorders workspaces |
| T51 | footer add | tap `Adicionar pasta ou workspace` | `Adicionar pasta…`, `Importar .code-workspace…`, `Novo workspace…` |
| T52 | new-workspace dialog | name `ATRIUM`, tick 2 folders, `Criar` | workspace with those folders in order |
| T53 | folder menu | `Adicionar a workspace` → `ATRIUM`, then `Tirar deste workspace` | added, then removed |
| T54 | workspace menu without file | open | `Associar .code-workspace…`, no `Abrir no vscode`; rename via dialog + Enter; `Desfazer workspace` keeps folders |
| T55 | workspace with file | open menu | `Abrir no vscode`, `Desassociar .code-workspace` |
| T56 | rail badge text `▶` | render | badge width < 24 |
| T57 | rail badge text `três arquivos` | render | badge width ≤ 32 (rail − 4) |
| T58 | banner shown | wait 5 s / 6 s | present / gone |
| T59 | sticky banner | wait 18 s | still present; `clearBanner` removes |
| T60 | banner, 5 s later a new one | wait 2 s then 6 s | second visible, then gone |
| T61 | sticky then normal banner | wait 5 s | normal banner shown (sticky replaced) |
| T62 | button firing confetti | tap; pump 1.2 s; pump 1.6 s | overlay present, still present, removed |
| T63 | confetti running | tap the button under it again at 300 ms | tap received (no pointer capture) |
| T64 | tab in collapsed workspace/folder | `revealTab` | workspace, folder (that appearance) and feature expanded; mirrored folder revealed in first workspace |
| T65 | loose tab | `revealTab` | revealed at root (no workspace) |
| T66 | sidebar hidden or plugin view shown | `revealTab` | sidebar visible in session view; already-visible list stays visible |
| T67 | search hides the tab / search matches it | `revealTab` | search cleared / kept |
| T68 | reveal | `takeRevealScroll` twice | true once, only for the revealed appearance |
| T69 | reveal then a banner | wait 3 s | highlight cleared |
| T70 | target row out of view in 300 px sidebar | reveal, 600 ms | row fully inside sidebar; list scrolled > 0 |
| T71 | pane header | tap `mostrar na lateral` | `revealedTabId` = that tab |
| T72 | tab spawned by a flow right after its parent | render sidebar | indented deeper than parent; unrelated tab not indented |
| T73 | flow child separated from parent by another row | render | not indented |
| T74 | flow editor on a working tab | pick `Continuar`, type, `Armar` | one keepGoing follow-up with that text |
| T75 | tab at rest | add `Comando` step | primary shows `Disparar`; tapping arms |
| T76 | tab with 2 subagents running after Stop | open flow | shows `2 agentes rodando` and `esperando os agentes que ela abriu` |
| T77 | new flow in folder `meu-repo` | open | `Abre uma sessão em meu-repo`; primary disabled until prompt; fill label+prompt, add `Outra sessão`, `Abrir e armar` → session opened with prompt/label/cwd; follow-up newSession prefilled with review prompt (contains `git diff`) |
| T78 | 3 panes in grid | panel menu `Agrupar painéis` | group `3 terminais`; menu then offers `Desagrupar "3 terminais"`, not `Agrupar`; ungroup keeps 3 tabs |
| T79 | grid no longer equals group | panel menu | offers both `Agrupar painéis` and `Desagrupar …` |
| T80 | single pane | panel menu | no `Agrupar painéis` |
| T81 | grouped, all dismissed | tap row `um` | grid restored `um, dois, tres` |
| T82 | group shown | tap an outside row | screen becomes that row only |
| T83 | saved group | render | `Grupos` ruler, group name and summary `3 terminais`; tap → grid |
| T84 | groups ruler | tap `Grupos` | collapsed (rows hidden, count `1`); persisted `groupsCollapsed: true` |
| T85 | 2 groups | clear button hidden until hover; tap | dialog `Esquecer os 2 grupos?`; `Cancelar` keeps; `Esquecer` clears + banner `2 grupos esquecidos…`; ruler disappears |
| T86 | 1 group | tap clear | cleared without dialog; banner mentions its name |
| T87 | finished notice for a renamed tab | render bell list | row shows current name + `terminou o trabalho` |
| T88 | 2 sessions, 1 unread | render status bar | `2 sessões`, bell count `1`; tap bell → list open |
| T89 | toast hovered | wait 16 s; unhover; wait 8 s | still present; then gone |
| T90 | toast | tap it | pane focused, toast gone, unread 0 |
| T91 | new toast | settle; dismiss | card right edge = window right, width 380; leaves sliding right, then removed |
| T92 | 2 notices | `Limpar tudo`, 100 ms | still 2 notices and same height; after settle: empty text, smaller height |
| T93 | clear-all running | close list mid-way | notices still cleared |
| T94 | session list vs bell | open one | the other closes |

---

## 7. Rust GUI approach

Requirements specific to Maestria: many simultaneous terminal grids (10–30 panes, a dozen
visible), heavy text with font fallback (Claude Code glyphs, emoji), custom chrome
(rounded cards with shadows, popovers, submenus, drag & drop, small animations), theme
switch at runtime, low idle CPU (status spinners are the only constant animation), fast
cold start, low RSS, Linux (X11 + Wayland) and macOS.

| Toolkit | Fit for this app |
|---|---|
| **gpui** (Zed) | Best fit. Zed's `terminal_view` already renders `alacritty_terminal` grids on gpui at high FPS with batched glyph atlases; GPU (Metal / Vulkan via blade) with on-demand frames (idle ≈ 0 % CPU); proper text shaping + fallback (CoreText / cosmic-text); rich styling (shadows, rounded corners, overlays, tooltips, drag-drop, animations) that maps 1:1 to the Flutter design; fast startup. Cons: API churns (pin a git rev / crate version), thin docs, few third-party widgets (we draw our own anyway), Linux needs Vulkan driver at runtime. |
| egui / eframe | Quick to build, tiny code; `egui_term` exists. But immediate mode re-lays out every visible widget each repaint; with 12 live terminals streaming output that is a lot of CPU; text stack (epaint/ab_glyph) has weak shaping/fallback; achieving the polished card look (soft shadows, popover animations) is fiddly. Good for prototypes, not the target. |
| iced | Elm architecture fits the store model; cosmic-text gives good shaping/fallback; `iced_term` (alacritty backend) exists. wgpu startup and memory are heavier (~+40–80 MB), custom overlays/submenus/drag-drop need more custom widget work, animation support is manual. Solid second choice. |
| slint | Declarative, very small footprint, great for forms. No terminal widget; a grid must be rasterized to an image or a custom renderer each frame — wrong shape for 12 streaming terminals. License (GPL/royalty-free/commercial) needs a decision. Not recommended. |

**Recommendation:** gpui + `alacritty_terminal` (pty via `portable-pty` or alacritty's tty)
with one `Model` per session and a single app-wide `Store` entity; terminals render only
dirty damage; status spinners driven by a 60 fps timer that runs only while a spinner is
visible. Fallback if gpui proves unworkable: iced + iced_term. Native bits: `rfd` (async
pickers, modal to the window), `objc2` for dock badge and Quick Look, `notify-rust` /
`notify-send` for notifications, `fontconfig`/`font-kit` for face discovery.

Suggested module layout (each file ≤ 400 LOC):

```
src/
  main.rs                    app bootstrap, window options (title, app_id, size), lifecycle (exit → shutdown, focus → set_window_active)
  native/{mod.rs, pickers.rs, dock_mac.rs, quicklook_mac.rs, notify.rs}
  theme/
    mod.rs                   Palette, Ansi, current theme handle, derived (shadow, on_accent, terminal colours, group_tints)
    builtin_dark_a.rs        themes 1–8
    builtin_dark_b.rs        themes 9–16
    builtin_light.rs         themes 17–19 + ALL order + retired aliases + by_id/is_builtin
    json.rs                  theme JSON parse/validate (error strings verbatim)
    oklch.rs                 sRGB<->OKLCH
    tint.rs                  MxTint enum, labels, hues, per-palette cache
    typography.rs            MxType, clamps, zoom, faces list, fallback chain, font discovery
  ui/
    style.rs                 shared metrics (gap, radius), text styles, tooltip delay, shadows
    icons.rs                 material icon font/SVG lookup, claude.svg, terminal.svg
    claude_avatar.rs         mark + status badges, spinner/pulse
    panel.rs                 MxPanel card, pane header box, pin button, key hint
    shell/{window.rs, status_bar.rs, banner.rs, grip.rs}
    rail.rs
    sidebar/{mod.rs, header.rs, search.rs, footer.rs, list.rs, workspace.rs, folder.rs,
             feature.rs, loose.rs, groups.rs, tab_row.rs, chips.rs, drag.rs, plugin_page.rs}
    menus/{menu.rs, submenu.rs, open_here.rs, panel_menu.rs, folder_menu.rs,
           worktree_menu.rs, workspace_menu.rs, feature_menu.rs, group_menu.rs,
           filter_menu.rs, tint_menu.rs}
    dialogs/{common.rs, prompt.rs, folder.rs, task.rs, worktree.rs, workspace.rs,
             groups.rs, feature.rs, move_to.rs, chat_history.rs, launcher.rs}
    flow_editor/{mod.rs, cards.rs, tiles.rs}
    settings/{mod.rs, rail.rs, appearance.rs, gallery.rs, typography.rs, shortcuts.rs,
              keycap.rs, launchers.rs, plugins.rs, sessions.rs, account.rs}
    notices/{center.rs, row.rs, toasts.rs, session_list.rs, bell.rs}
    anim/{tween.rs, confetti.rs}
packaging/linux/{br.com.marrow.maestria.desktop, debian/postinst, debian/postrm, Dockerfile, smoke-test.sh}
packaging/macos/{make-dmg.sh, Info.plist.tmpl, entitlements.plist}
.github/workflows/release.yml
```

Golden tests worth porting first: T1–T13 (pure theme math, incl. the tint reference table
above), T16–T24 (typography), T58–T61 (banner timing), then the menu/sidebar behaviours
through store-level tests.
