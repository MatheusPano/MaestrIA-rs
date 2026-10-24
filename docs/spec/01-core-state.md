# 01 — Core state & persistence (behavioral spec)

Source of truth (read-only): `/home/lucas/maestria/repo`
- `lib/services/store.dart` (the `AppStore`, 5200 lines)
- `lib/models.dart`
- `lib/services/{workspace,layout,history,paths,shell,git,editor,agents}.dart`
- tests listed in §5, and `docs/superpowers/specs/*.md`

This document is meant to be sufficient to rebuild the subsystem in Rust without
reading the Dart. Where the Dart has a quirk that matters for compatibility, it is
called out as **QUIRK**. Where a behavior belongs to another subsystem (PTY, hooks
server, plugins, UI), it is described only at the interface the store uses and
marked **(xref)**.

Conventions:
- "notify" = the store's `notifyListeners()` (UI repaint signal). In Rust: emit a
  "changed" event. Every notify also recomputes `agents.eager` (see §3.17).
- "save" = the debounced config write `_save()` (§2.3). Not a direct write.
- All user-visible strings are Portuguese and are quoted verbatim. `…` is U+2026,
  `—` is U+2014, `▸` is U+25B8, `·` is U+00B7.
- **String lengths**: Dart `String.length`/`substring` count **UTF-16 code units**.
  Every clip below (32, 70, 72, 90, 4000, 28/27, 128 KiB, 24, 60) is in UTF-16 units.
  Rust must clip by UTF-16 units to match exactly (only differs for non-BMP chars).
- Dart `jsonEncode` preserves map insertion order; write keys in the order given
  here (use `serde_json` with `preserve_order`) so diffs of `config.json` stay
  stable.

---

## 1. Domain model

### 1.1 `Folder` (a repo / directory in the sidebar)

| field | type | default | persisted | notes |
|---|---|---|---|---|
| `root` | String | required | yes `root` | Identity. Main checkout path (worktrees resolve back to it). Immutable. |
| `name` | String | required | yes `name` | Display name, editable. |
| `collapsed` | bool | false | yes `collapsed` (always written) | Fold state when drawn **standalone at root**. Inside a workspace, fold state lives in `Workspace.collapsedFolders`. |
| `tint` | `MxTint?` | null | `tint` only if set | Folder colour. |
| `isLoose` | bool | false | no | Only true for the single loose tray folder. |
| `isRepo` | bool | false | no | Set by `refreshGit`: `git worktree list` returned ≥1 entry. |
| `branch` | String | `''` | no | Branch of the main worktree (from `refreshGit`). |

**Loose tray** (`Folder.loose`): exactly one per store, `root = $HOME` (or `/` if
HOME unset), `name = 'Avulsos'`, `isLoose = true`, `collapsed = false`. Never in
`folders`, never persisted, never git-refreshed, never removable. Panels that
belong to no repo hang here. Plugin views and sidebar plugin tabs also use it as
their folder.

### 1.2 `Workspace` (a named group of folders, first-level sidebar section)

| field | type | default | persisted |
|---|---|---|---|
| `id` | String | required, stable | `id` |
| `name` | String | required, non-empty | `name` |
| `collapsed` | bool | false | `collapsed: true` only when true |
| `tint` | `MxTint?` | null | `tint` only if set |
| `codeWorkspacePath` | String? | null | `codeWorkspacePath` only if set |
| `folderRoots` | List<String> (ordered) | [] | `folderRoots` (always written) |
| `collapsedFolders` | ordered Set<String> | {} | `collapsedFolders` (list) only if non-empty |

Invariants:
- Does **not** own sessions. A folder may be in several workspaces ("espelho"/mirror);
  each workspace keeps its own order and its own per-appearance fold state.
- A workspace may be empty (created manually, last folder removed) — it stays.
- `folderRoots` may contain roots that are no longer in `folders`; they are skipped
  when resolving (`foldersOf`).

### 1.3 `FeatureOrHotfix` (named job inside one folder; formerly "project")

| field | type | default | persisted |
|---|---|---|---|
| `id` | String | required | `id` |
| `folderRoot` | String | required | `folderRoot` |
| `name` | String | required | `name` |
| `brief` | String | `''` | `brief` (always) |
| `collapsed` | bool | false | `collapsed` (always) |
| `kind` | `FeatureOrHotfixKind` | feature | `kind` (always, `"feature"`/`"hotfix"`) |
| `tint` | `MxTint?` | null | `tint` only if set |

- `folderRoot` may be the loose tray's root (a job not in any repo).
- `brief` is passed to every Claude session launched in it as `--append-system-prompt`.
- A feature never spans two folders; panels of other folders cannot join it.

`FeatureOrHotfixKind` (enum name is what is persisted; unknown/missing → `feature`):

| kind | label | newLabel | the | inThis | thisOne | ofThis | ofThe | dissolved | icon |
|---|---|---|---|---|---|---|---|---|---|
| feature | `feature` | `Nova feature` | `a feature` | `nessa feature` | `essa feature` | `dessa feature` | `da feature` | `Feature dissolvida` | track_changes |
| hotfix | `hotfix` | `Novo hotfix` | `o hotfix` | `nesse hotfix` | `esse hotfix` | `desse hotfix` | `do hotfix` | `Hotfix dissolvido` | bolt |

`other` = the opposite kind.

### 1.4 `PaneGroup` (a saved arrangement of panes)

| field | type | persisted |
|---|---|---|
| `id` | String | `id` |
| `name` | String | `name` |
| `panes` | List<Map> — one **recipe** per pane (§2.5), in on-screen order | `panes` |
| `tree` | JSON tree (§2.6) whose leaves are indices into `panes`; null = single pane | `tree` only if non-null |

Derived:
- `count = panes.length`.
- `summary = summarize(panes)`: count recipes by `kind`:
  `claude` → `'uma sessão'` / `'<n> sessões'`; `shell` → `'um terminal'` /
  `'<n> terminais'`; `reader` → `'um leitor'` / `'<n> leitores'`. Parts in that
  order, omitting zero. 0 parts → `''`; 1 → the part; else
  `'<p1>, <p2> e <last>'` (comma-join all but last, then `' e '` + last).
- `where`: for each recipe, root = `'avulsos'` if `loose == true`, else
  `folderRoot ?? ''`; take last non-empty path segment; unique in order; join `' · '`.
- `color = Mx.groupTints[sum(UTF-16 code units of id) % 4]`, groupTints =
  `[ansi.cyan, ansi.magenta, ansi.blue, ansi.brightGreen]` of the current palette (xref theme).
- Not owned by a folder; it owns nothing.

### 1.5 `Launcher` (user-taught program) and `LauncherIcon`

| field | type | persisted |
|---|---|---|
| `id` | String, stable | `id` |
| `name` | String (trimmed) | `name` |
| `command` | String (trimmed), run via login shell as typed | `command` |
| `icon` | LauncherIcon | `icon` (enum name) |

`LauncherIcon` (name → label): `terminal`→`Terminal`, `monitor`→`Monitor`,
`chart`→`Gráfico`, `git`→`Git`, `server`→`Servidor`, `database`→`Banco`,
`rocket`→`Foguete`, `bug`→`Bug`, `edit`→`Editor`, `box`→`Caixa`. Unknown/missing → `terminal`.
`color` = same formula as `PaneGroup.color` on the launcher id.

### 1.6 `MxTint` (user-chosen colour names, persisted by name)

`red, orange, yellow, lime, green, aqua, cyan, petrol, violet, magenta, pink, brown, grey`
(labels: Vermelho, Laranja, Amarelo, Lima, Verde, Verde-água, Ciano, Azul-petróleo,
Violeta, Magenta, Rosa, Marrom, Cinza). Unknown name → null (no colour; e.g. a
retired `blue` silently reads as null). Colour computation is theme's job (xref).

### 1.7 `WorktreeInfo`

`path: String`, `branch: String` (`'(detached)'` when no branch line),
`isMain: bool` (first entry of `git worktree list`), `prunable: bool`.
`shortLabel`: first match of regex `([A-Z]+[#-]?\d+)` in branch; else last `/`
segment of branch; if that is empty, last segment of path.

### 1.8 `MxTab` (one panel)

`TabKind` enum: `shell, claude, reader, setup, plugin` (persisted by name).

| field | type | default | persisted? | notes |
|---|---|---|---|---|
| `id` | String | `_nextId()` | no (layout uses list index) | see §1.17 |
| `folder` | Folder (by reference) | required | as `folderRoot` + `loose` | rename of folder renames panel |
| `kind` | TabKind | required | `kind` | |
| `cwd` | String | required (immutable) | `cwd` | |
| `branch` | String | `''` | no | filled async (`git rev-parse`) |
| `customLabel` | String? | null | `label` | |
| `tint` | MxTint? | null | `tint` | |
| `term` | TermSession | new | no | PTY (xref) |
| `hooks` | HookState | new | partially (touched/plans/suggestions) | |
| `context` | ContextUsage? | null | no | from statusline |
| `doc` | MxDoc? | null | `doc` | non-null iff kind==reader (xref reader) |
| `setup` | MxSetup? | null | `setup` | non-null iff kind==setup (xref) |
| `view` | PluginView? | null | never (plugin tabs never saved) | xref plugins |
| `owner` | String? | null | no | plugin id that owns this terminal |
| `ownerTag` | String? | null | no | |
| `embedded` | bool | false | no | terminal inside a plugin window, never in pane tree |
| `launcher` | Launcher? (by ref) | null | `launcher` (id only) | becomes null when launcher deleted |
| `featureOrHotfixId` | String? | null | `featureOrHotfixId` | |
| `bornOf` | String? | null | no | parent tab id when opened by a flow step |
| `armed` | bool | false | no | flow queue waiting to fire |
| `followUps` | List<FollowUp> | [] | **never** | |
| `sessionId` | String? | null | via `sessionId` (if resumable) | from `claude agents` or hooks |
| `agentName` | String? | null | no | `--name` used at launch / reported by CLI |
| `dirty` | int | 0 | no | `git status --porcelain` line count |
| `startedAt` | DateTime | now at construction | no | |
| `zoom` | int | 0 | `zoom` if ≠0 | font steps |
| `groupId` | String? | null | `group` (layout only) | |
| `done` | bool | false | `done: true` (layout only) | manual "concluída" mark |
| `pinned` | bool | false | `pinned: true` (layout only) | |
| `onSessions` | bool | false | no | plugin window moved to sessions screen |
| `hibernated` | bool | false | `hibernated: true` (layout only) | |
| `restedAt` | DateTime? | null | no | instant of transition to rest |
| `unseen` | bool | false | no | stopped while you weren't watching |

Derived:
- `isReader = kind==reader && doc!=null`; `isSetup = kind==setup && setup!=null`;
  `isPluginView = kind==plugin && view!=null`; `isPassive = isReader||isSetup||isPluginView`.
- `exited = term.exited`.
- `resumeId = sessionId ?? hooks.sessionId`.
- `resumable = kind==claude && resumeId != null`.
- `status`: reader/setup/plugin → `unknown`; shell → `ended` if exited else `unknown`;
  claude → `hooks.status`.
- `rested = restedAt != null && !exited && status.atRest`.
- `restedAgo = rested ? shortAgo(restedAt) : null`.
- `folderRoot = folder.root`.
- `fontSize = Mx.type.sizeAt(zoom)` (xref theme).

`title` (first match wins):
1. `customLabel` if non-null and non-empty.
2. reader: `doc.title`.
3. setup: `'Claude · <basename of setup.root>'`.
4. plugin view: `view.title`.
5. launcher: `launcher.name`.
6. loose folder: `'~'` if `cwd == folder.root`, else last `/` segment of cwd.
7. `cwd == folder.root` → `folder.name`.
8. branch non-empty and ≠ `'(detached)'`: first match of `([A-Za-z]+[#-]?\d+)` in
   branch, else last `/` segment of branch.
9. last `/` segment of cwd.

`subtitle` (first match wins):
1. reader: if `doc.missing` → `'<source.label> · o arquivo não está mais lá'`; else
   join with `' · '`: `source.label`, then `'de <origin>'` only if origin non-empty and
   `title` does not contain origin, then `HH:MM` of `doc.at` (zero-padded).
2. setup: `setup.selected ?? 'Configuração do Claude'`.
3. plugin view: `'Plugin · <pluginName>'`.
4. `hibernated` → `'Hibernada · clique pra retomar'`.
5. `exited` → `'Processo saiu (<exitCode or ?>)'`.
6. launcher → `launcher.command`.
7. kind shell → `'Shell'`.
8. else `hooks.subtitle` (§1.9).

### 1.9 `HookState` and `ClaudeStatus`

`ClaudeStatus` enum (order matters for nothing persisted): `unknown, starting, ready,
working, tool, waitingInput, waitingAnswer, waitingPermission, idle, ended`.

| status | label | atRest | needsHuman | callToAction |
|---|---|---|---|---|
| unknown | `—` | | | `te espera` |
| starting | `Abrindo` | | | `te espera` |
| ready | `Pronto` | | | `te espera` |
| working | `Pensando` | | | `te espera` |
| tool | `Rodando` | | | `te espera` |
| waitingInput | `Esperando você` | **yes** | **yes** | `te espera` |
| waitingAnswer | `Pergunta` | | **yes** | `te faz uma pergunta` |
| waitingPermission | `Permissão` | | **yes** | `quer aprovação` |
| idle | `Ocioso` | **yes** | | `te espera` |
| ended | `Encerrada` | | | `te espera` |

Colours (xref theme): working/tool accent; waitingInput/waitingAnswer yellow;
waitingPermission red; idle/ready green; starting purple; others faint.

`HookState` fields: `status` (initial **starting**), `sessionId?`, `permissionMode?`,
`activeTool?`, `toolStartedAt?`, `lastToolTarget?`, `lastMessage?` (clipped 90),
`lastMessageFull?`, `question?`, `questions:int=0`, `lastPrompt?` (clipped 70),
`prompts:int`, `tools:int`, `lastEventAt?`, `forksOut:int=0`, `forkIds:Set<String>`,
`touched: List<String>` (max **200**, drop oldest), `plans: List<PlanNote>` (max **8**),
`suggestions: List<SuggestedTask>` (max **20**). `plan = plans.last`. `busyForks = forksOut > 0`.

`settle()`: if status == starting → status = ready, return true; else false.

`subtitle`:
- waitingPermission → `'Quer aprovação pra <activeTool>'` (Dart interpolates null as `null`).
- waitingAnswer → question null ? `'Te faz uma pergunta'` : `'Pergunta: <question><rest>'`
  where rest = `' (+<questions-1>)'` if questions > 1 else `''`.
- waitingInput → `lastMessage ?? 'Te espera'`.
- tool with activeTool → `'<activeTool><(target)><secs>'`: target = `'(<lastToolTarget>)'`
  or `''`; secs = `' <seconds since toolStartedAt>s'` or `''`.
- working → `lastPrompt ?? 'Trabalhando'`.
- idle → `lastMessage ?? 'Pronto'`.
- else `status.label`.

**Status state machine** (HookReducer, xref hooks spec — summarized because store tests
depend on it). Every event sets `lastEventAt = now`, `sessionId = payload.session_id ?? old`,
`permissionMode = payload.permission_mode ?? old`.
- Event with non-empty `agent_id` (subagent/fork): if `SubagentStop` → remove id from
  `forkIds`, `forksOut = max(0, forksOut-1)`; else add id, and if `forkIds.length >
  forksOut` → `forksOut = forkIds.length`. `PreToolUse` → `tools++`; `PostToolUse` →
  record touched file. **Status unchanged.** Return.
- `SubagentStop` without agent_id → `forksOut = max(0, forksOut-1)`. Return.
- `SessionStart` → ready; clear forks.
- `UserPromptSubmit` → question=null, questions=0, prompts++, **working**,
  lastPrompt = clip70(`prompt` ?? `user_input`).
- `PreToolUse` → if tool `AskUserQuestion`: record question; if tool in {`Task`,`Agent`}:
  forksOut++; if `ExitPlanMode`: append plan (dedupe equal to last; cap 8); tools++;
  status = `waitingAnswer` if AskUserQuestion else **tool**; activeTool, toolStartedAt=now,
  lastToolTarget.
- `PostToolUse` → **working**, clear activeTool/toolStartedAt/question; record touched.
- `PostToolUseFailure` → clear tool & question; `idle` if `is_interrupt == true` else `working`.
- `StopFailure` → **idle**, clear tool, lastMessage = clip90(error) ?? old.
- `Notification`: `permission_prompt` → waitingAnswer if activeTool==AskUserQuestion else
  waitingPermission; `agent_needs_input` → waitingInput, lastMessage = clip70(message) ?? old;
  `idle_prompt` → idle unless status.needsHuman.
- `Stop` → **idle**, clear tool, lastMessage = clip90(`last_assistant_message`) ?? old,
  lastMessageFull = trimmed message if non-blank.
- `SessionEnd` → reason (`reason` ?? `end_reason` ?? `'other'`) in {resume, clear} → ready, else
  **ended**; clear forks.

Store-side transitions (not in reducer):
- launch → `starting`; 3 s after `_launchClaude`, if tab still exists and not exited,
  `settle()` (starting→ready).
- `hibernate` → `ended`; `wake` → `starting`.
- restored-hibernated panel → `ended`.

### 1.10 `FollowUp`, `FollowUpKind`, `FlowHold`

`FollowUp { kind, text, targetTabId? }` — JSON `{kind, text, target?}` (only used by
UI/flow editor; never in config). `fromJson`: unknown kind → null; `text` default `''`.

| kind | label | short | blurb | hint | handsBack |
|---|---|---|---|---|---|
| keepGoing | `Continuar aqui` | `Continuar` | `Manda o texto de volta pra esta mesma sessão` | `O que mandar pra ela em seguida` | **true** |
| newSession | `Abrir outra sessão` | `Outra sessão` | `Abre uma sessão nova na mesma pasta, já com esse prompt` | `O prompt com que a sessão nova abre` | false |
| command | `Rodar um comando` | `Comando` | `Sobe um terminal rodando a linha abaixo` | `flutter analyze && flutter test` | false |
| handoff | `Passar pra outro painel` | `Passar a bola` | `Entrega o recado final desta sessão a outro painel` | `O recado que vai junto com a última mensagem dela` | false |

`FlowHold`: `go`→`disparando`, `working`→`a sessão ainda está trabalhando`,
`forks`→`esperando os agentes que ela abriu`, `quiet`→`confirmando que ela parou mesmo`.

### 1.11 Notices, plans, suggestions

`MxNoticeKind`: `question` (verb `te fez uma pergunta`), `permission` (`quer aprovação`),
`finished` (`terminou o trabalho`), `suggested` (`sugeriu uma tarefa`).
`MxNotice { tabId, kind, title (tab title at creation), at=now, suggestionId?, detail?, read=false }`.
In memory only.

`PlanNote { text, at=now }`. `headline`: first line that, after `trim()` and removing a
leading `^#+\s*`, is non-empty; if length ≤ 70 return it else first 70 + `…`;
none → `'Plano sem título'`. JSON `{text, at: ISO8601}`; `fromJson`: not a map, or text
null/blank → null; `at` = tryParse or now.

`SuggestedTask { id ('task_' + 8 lowercase hex), title, tldr, prompt, at }`. JSON
`{id,title,tldr,prompt,at}`. `fromJson`: not a map, id null, or prompt null/blank → null;
title/tldr default `''`.

`SuggestionChoice { worktree, local, here, dismiss }`. `SuggestionEnd { started, dismissed }`
(defined in task_mcp, xref).

### 1.12 `AgentInfo` (one row of `claude agents --json`) and `ContextUsage`

`AgentInfo.fromJson`: `pid` (int?), `cwd` (String, default `''`), `kind` (default
`'interactive'`), `sessionId?`, `name?`, `status?`, `waitingFor?`, `state?`,
`startedAt` (num → int epoch ms).

`ContextUsage { percent, tokens, window }` from statusline JSON: requires
`context_window` map with `used_percentage` num, `context_window_size` num > 0,
`current_usage` map; tokens = sum of `input_tokens`, `cache_creation_input_tokens`,
`cache_read_input_tokens` (non-num → 0); all rounded. Else null. Memory only.

### 1.13 Pane tree (`layout.dart`)

```
PaneNode = PaneLeaf { tabId: String (mutable) }
         | PaneSplit { axis: row|column, children: Vec<PaneNode>, weights: Vec<f64> (sum 1) }
DropSide = left|right|top|bottom|center ; axis(left/right)=row, (top/bottom)=column ;
           leading = left|top
```
Constants: `minWidth = 240`, `minHeight = 140`; `minFor(row)=240, minFor(column)=140`.

Operations (all mutate in place unless stated; all addressed by tab id):
- `order(root)`: DFS leaves left→right/top→bottom.
- `has(root,id)`, `count(root)`.
- `swap(root, targetId, tabId)`: the leaf showing targetId now shows tabId (no-op if absent).
- `insert(root, tabId, targetId, side)` → new root:
  - root is leaf: if leaf.tabId ≠ target return root unchanged; else new split
    `(side.axis, leading ? [new, root] : [root, new], [0.5,0.5])`.
  - root is split: DFS; at the target leaf at index i of parent split S:
    if `S.axis == side.axis`: insert new leaf at `i` (leading) or `i+1`, and the target's
    weight is halved, both get `weights[i]/2`;
    else replace the leaf with `split(side.axis, leading?[new,leaf]:[leaf,new], [.5,.5])`.
    Returns root (unchanged if not found).
- `remove(root, tabId)` → new root or null: leaf→null if matches. In a split, remove the
  first branch that changed: if child became null, remove child + its weight and spread
  the freed weight over the others proportionally (if remaining sum ≤ 0, equalize);
  else replace child and **absorb**: if the new child is a split with the same axis as
  the parent, inline its children with weights `share * childWeight`. A split left with
  one child collapses to that child.
- `edge(root, tabId, side)`: if root is split with `root.axis == side.axis`: scale all
  weights by `n/(n+1)`, insert leaf at 0 (leading) or end with weight `1/(n+1)`; else
  `split(side.axis, leading?[leaf,root]:[root,leaf], [.5,.5])`.
- `copy(node)`: deep copy.
- `sameShape(a,b)`: same structure, axes, child counts, leaf ids; weights ignored.
- `resize(split, gutter, delta, total)`: ignore if `gutter<0 || gutter+1 >= len || total<=0`.
  `floor = clamp(minFor(axis)/total, 0, 0.45)`; `a=w[g]`, `b=w[g+1]`; `lower=floor-a`,
  `upper=b-floor`; if `lower>upper` return; `moved = clamp(delta/total, lower, upper)`;
  if 0 return; `w[g]=a+moved`, `w[g+1]=b-moved`.
- JSON (de)serialization: §2.6.

### 1.14 Search filters

`MxFilterGroup`: `kind` (`Tipo`), `state` (`Estado`).
`MxFilter` (label, group, matches(tab)):
- `claude` (`Sessões do Claude`, kind): `kind == claude`
- `shell` (`Terminais`, kind): `kind == shell`
- `waiting` (`Esperando você`, state): `!done && status.needsHuman`
- `working` (`Rodando`, state): `status ∈ {working, tool}`
- `parked` (`Paradas`, state): `!done && (status ∈ {idle, ready, ended} || exited)`
- `finished` (`Concluídas`, state): `done`

### 1.15 `AppStore` state

| field | type | default | persisted key |
|---|---|---|---|
| `folders` | List<Folder> | [] | `folders` |
| `workspaces` | List<Workspace> | [] | `workspaces` |
| `rootOrder` | List<String> (`workspace:<id>` / `folder:<root>`) | [] | `rootOrder` (written as the *derived* sidebar order) |
| `featuresOrHotfixes` | List<FeatureOrHotfix> | [] | `featuresOrHotfixes` |
| `loose` | Folder | Folder.loose($HOME or `/`) | — |
| `tabs` | List<MxTab> (order = sidebar order and ⌘1..9) | [] | `layout.panes` (filtered) |
| `groups` | List<PaneGroup> | [] | `groups` |
| `launchers` | List<Launcher> | [] | `launchers` |
| `worktrees` | Map<root, List<WorktreeInfo>> | {} | — |
| `panes` | PaneNode? (tree of the visible screen) | null | `layout.tree` |
| `focusedPaneId` | String? | null | `layout.focused` (as index) |
| `_screen` | String? (null = sessions; else plugin id) | null | — |
| `_parked` | Map<String?, (tree, focus)> | {} | sessions tree saved even if parked |
| `windowActive` | bool | **true** | — |
| `_awaySince` | DateTime? | null | — |
| `banner` | String? | null | — |
| `notices` | List<MxNotice> newest first, cap **50** | [] | — |
| `noticesOpen`, `sessionsOpen` | bool | false | — |
| `toasts` | List<MxNotice> cap **3** | [] | — |
| `doNotDisturb` | bool | false | `doNotDisturb` |
| `suggestionsToPlugin` | bool | false | `suggestionsToPlugin` |
| `sidebarWidth` | f64 | **352** (min 248, max 680) | `sidebarWidth` |
| `sidebarHidden` | bool | false | `sidebarHidden` |
| `statusBarTop` | bool | false | `statusBarTop` |
| `railOrder` | List<String> | [] | `railOrder` |
| `sidebarView` | String? | null | `sidebarView` |
| `groupsCollapsed` | bool | false | `groupsCollapsed` |
| `hibernateMinutes` | int | **30** (0 = off, max 1440) | `hibernateMinutes` |
| theme (global `Mx.palette`) | id | `'maestria'` | `theme` |
| typography (`Mx.type`) | MxType | mono `Hack`, size 13.0, line 1.2 | `type` |
| `keymap` | MxKeymap | defaults | `shortcuts` |
| `plugins` | Plugins | — | `plugins` |
| `floats` | PluginFloats | — | `floats` |
| `query`, `filterRoots`, `filterFeaturesOrHotfixes`, `filterFlags` | search | empty | **never** |
| `revealedTabId`, `revealedWithin` | reveal highlight | null | — |
| `pluginModal`, `_sidebarTabs`, `sidebarBadges` | plugin UI | — | — (xref plugins) |
| `_seq` | int | 0 | — |
| `_run` | String base36(micros at store creation) | — | — |
| `_restoring`, `_quitting`, `_gone` | bool | false | — |
| `_alerted` | Set<String> | {} | — |
| `_settled` | Map<taskId, SuggestionEnd> | {} | — |
| `_tracking` | Map<suggestionId,(plugin,tabId,choice)> | {} | — |
| `_hole` | (tree, id)? | null | — |

### 1.16 Lifecycle state machines

**Tab process lifecycle (claude)**
```
openClaude(start=true) ──launch──> [running: status starting] --3s settle--> ready
   │                                    │ hooks → working/tool/waiting*/idle/ended
   │                                    │ process exits → exited (status per hooks)
   │ openClaude(start=false) ──> [hibernated: exited(parked), status ended]
[running] --hibernate()--> [hibernated] --wake()/select()/openGroup--> [running, starting]
any --closeTab()--> removed from tabs (process SIGHUP→3s→SIGKILL, async)
```
- `canHibernate = kind==claude && !exited && !hibernated && resumeId != null`.

**Done mark**: `setDone(true)` → done, drop queue; `UserPromptSubmit` hook → done=false.
Never set automatically.

**Rest/unseen**: on transition `!before.atRest → after.atRest`: `restedAt = now`,
`unseen = !watching(tab)`, and `armed = true` if `followUps` non-empty. On
`before.atRest → !after.atRest`: `restedAt = null`, `unseen = false`. A second `Stop`
(idle→idle) is not a transition (timestamp kept). `seeFocused` (1 s tick) clears
`unseen` for the watched tab. `hibernate` clears both.

**Flow queue**: `queue(steps, now)` sets steps and `armed = steps.nonEmpty && (now || armed)`.
Arming on rest transition (above). 1 s tick `pumpFlows` → `holdFor == go` → `advance`.
`advance` pops one step; `armed = !step.kind.handsBack && followUps.nonEmpty`.
Disarmed by: closeTab, setDone(true), hibernate, shutdown/dispose.

**Pinned**: `togglePin` only while on screen. `isPinned = pinned && isOpen`. Every `_save`
clears `pinned` on tabs not placed in their screen tree.

**Active group**: derived — `activeGroup` = group of first open pane if *every* open pane
has that `groupId`.

**Notices**: unread → read via `readNoticesOf` (watching ≥1 s, opening, returning to work).

### 1.17 Identifier formats

| entity | format | example |
|---|---|---|
| tab | `'tab' + _run + '_' + _seq++` where `_run` = base36(µs since epoch at store creation) | `tabm3x9k2a1b_0` |
| sidebar plugin tab | `'sidebar:<pluginId>'` | |
| feature/hotfix | `'pj' + base36(µs now)` | |
| group | `'gp' + base36(µs now)` (on update keeps existing id) | |
| launcher | `'lch' + decimal(µs now)` | `lch1729500000000000` |
| workspace | `'ws' + base36(µs now) + base36(globalSeq++)` (global counter, process-wide) | |
| suggestion | `'task_' + 4 random bytes as 2-digit lowercase hex` (secure RNG) | `task_0a1bc2d3` |

Base36 is lowercase (`0-9a-z`).

### 1.18 Time/format helpers

`shortAgo(at, now)`: `since = now-at`; `<60s` → `agora`; `<60min` → `<m>min`; `<24h` →
`<h>h`; `<7d` → `<d>d`; else `'<day>/<month>'` (no padding) of `at`.

`shortUntil(at, now)`: `left = at-now`; negative → `agora`; `<1min` → `<1min`;
`<60min` → `<m>min`; `<24h` → `<h>h`; `<7d` → `<d>d`; else `'<day>/<month>'`.

`ChatEntry.ago`: `<1min` → `agora`; `<60` → `há <m> min`; `<24h` → `há <h> h`; days==1 →
`ontem`; `<7` → `há <d> dias`; else `<day>/<month>`.
`ChatEntry.size`: `<1024` → `<n> B`; `<1 MiB` → `<round(n/1024)> kB`; else
`<n/MiB with 1 decimal> MB`.
`ChatEntry.label`: title ≤32 → title, else first 32 + `…`. `where` = `'?'` if cwd empty
else last segment.

ISO-8601 timestamps written by Dart `DateTime.now().toIso8601String()` are **local time,
no offset**, `YYYY-MM-DDTHH:MM:SS.mmm[uuu]` (µs digits present when non-zero). Readers
use a lenient parse (accept with/without fraction, with `Z` or offset). Rust: write
`%Y-%m-%dT%H:%M:%S%.6f` in local time; parse leniently; on failure use now.

---
## 2. On-disk files

### 2.1 State directory

```
mxStateHome = if env FLUTTER_TEST is set: "<systemTemp>/maestria-test-<pid>"
              else $HOME (or "/" if unset)
mxStateDir  = mxStateHome + "/.maestria"
```
Rust: replace `FLUTTER_TEST` with an explicit override (e.g. env `MAESTRIA_STATE_HOME`
or a constructor parameter) so tests never touch the real config. The production path
**must** stay `~/.maestria/`.

Files in `~/.maestria/`:

| path | owner | format | this spec |
|---|---|---|---|
| `config.json` | store | JSON object (§2.2) | **yes** |
| `config.json.<pid>.tmp` | store | transient temp for atomic write | yes |
| `config.json.bak` | store | byte copy of a config that failed to load | yes |
| `hook-port` | hooks server | decimal port number | xref hooks |
| `statusline.sh` | hooks server | sh script | xref hooks |
| `plugins/` | plugins | installed plugins | xref plugins |
| `plugin-data/<id>/` | plugins | per-plugin data | xref plugins |
| `models/ggml-large-v3-turbo.bin` | dictation (disabled) | — | n/a |

Read-only external inputs of this subsystem:
- `~/.claude/projects/<escaped-cwd>/<sessionId>.jsonl` (chat history, §2.9).
- `*.code-workspace` files (JSONC, §2.10).

### 2.2 `config.json` — top-level schema

Written by `_writeConfig` with keys in exactly this order; "omit" means the key is absent
when the condition is false.

| key | type | written when | read default / handling |
|---|---|---|---|
| `folders` | `Folder[]` | always | see legacy `projects` |
| `workspaces` | `Workspace[]` | non-empty | `[]` |
| `rootOrder` | `string[]` | always — **the derived current `sidebarRows` keys**, not the raw field | `[]`; non-strings dropped |
| `featuresOrHotfixes` | `FeatureOrHotfix[]` | always | falls back to legacy `projects` |
| `groups` | `PaneGroup[]` | non-empty | invalid entries dropped |
| `launchers` | `Launcher[]` | non-empty | invalid entries dropped |
| `sidebarWidth` | number (double, e.g. `352.0`) | always | clamp to [248, 680]; absent → 352 |
| `sidebarHidden` | `true` | when true | false |
| `statusBarTop` | `true` | when true | false |
| `railOrder` | `string[]` | non-empty | `[]`; non-strings dropped |
| `sidebarView` | string | non-null | null |
| `doNotDisturb` | `true` | when true | false |
| `suggestionsToPlugin` | `true` | when true | false |
| `floats` | `{<floatId>: {corner, dx, dy}}` | non-empty | xref plugins (`dx`,`dy` rounded doubles; invalid entries dropped) |
| `groupsCollapsed` | `true` | when true | false |
| `hibernateMinutes` | int | when ≠ 30 | 30 (`num` → toInt) |
| `theme` | string palette id | always | unknown id → retired map (`tokyo-night→maestria`, `catppuccin-frappe→catppuccin-mocha`, `rose-pine→catppuccin-mocha`, `chatgpt-dark→graphite`) → else `maestria` |
| `type` | `{mono?, size?, line?}` | non-empty (only fields ≠ defaults Hack/13.0/1.2) | size clamp [9,24], line clamp [1.0,1.8], rounded to 2 decimals |
| `shortcuts` | `{<actionId>: [<chordId>...]}` | non-empty (only actions ≠ defaults) | xref shortcuts |
| `plugins` | `{disabled?: string[] (sorted), settings?: {<pluginId>: {..}}}` | non-empty | xref plugins |
| `layout` | `{panes, tree?, focused}` | always | §2.4 |

Booleans are **never** written as `false`; absent ⇒ default. Readers must accept
explicit `false` too.

**Removed keys**: `dictation` (feature disabled; dropped on next save — do not read/write).

### 2.3 Write algorithm (`_save` / `_writeConfig`)

`_save()`:
1. If `_restoring || _gone || _quitting` → return (no write scheduled).
2. For every tab with `pinned` that is not placed in the tree of its own screen
   (`_placed`), set `pinned = false`.
3. Cancel any pending debounce timer; schedule `_writeConfig` in **400 ms**.

`_writeConfig()` (all errors swallowed):
1. `mkdir -p` the state dir.
2. `open` = tabs where `!isPluginView && owner == null && (!exited || resumable)`, in
   `tabs` order.
3. `tree = Panes.toJson(<sessions-screen tree>, id → index in open or -1)`; the sessions
   tree is `panes` if `_screen == null`, else `_parked[null].tree`.
4. `focused` = `focusedPaneId` if on sessions screen else `_parked[null].focus`.
5. Serialize the object of §2.2 with `layout = {panes: open.map(toJson), tree (omit if
   null), focused: index of focused in open (−1 if none)}`.
6. Write to `config.json.<pid>.tmp` in the same dir, then `rename` over `config.json`
   (atomic). `<pid>` = this process id.

`touch()` = save + notify (for panes that change internally, e.g. setup editor).

`shutdown()`: set `_quitting = true`; if a debounce is pending, cancel it and run
`_writeConfig` **now** (awaited) — must happen before any process is killed, because
dead shells would otherwise drop from the layout. Then stop agents watcher, stop hooks
server, `await` plugins.stopAll() and `_end(tab)` for every tab in parallel
(`armed=false`, `term.kill()`).

`dispose()`: `_gone = true`; cancel save debounce (pending write is **discarded**),
banner timer, reveal timer, toasts; floats.dispose; agents.stop; hooks.stop;
plugins.killAll; every tab `armed=false`, `term.hangUp()` (SIGHUP, no wait). After
`_gone`, notify is a no-op.

### 2.4 Load algorithm (`_loadConfig`, called from `init`)

```
if !exists(config.json): return
try:
  j = jsonDecode(read(config.json)) as object
  readSidebar(j)                               // §2.7, legacy-aware
  groups    += j.groups    .map(PaneGroup.fromJson).nonNull
  launchers += j.launchers .map(Launcher.fromJson).nonNull     // before layout!
  sidebarWidth = clamp(j.sidebarWidth as num, 248, 680) if present
  sidebarHidden, statusBarTop, railOrder, sidebarView, doNotDisturb,
  suggestionsToPlugin, floats.readJson(j.floats), groupsCollapsed,
  hibernateMinutes = (j.hibernateMinutes as num)?.toInt() ?? 30
  plugins.load(j.plugins); plugins.scan()      // before theme (plugin themes)
  theme = byId(j.theme); type = MxType.fromJson(j.type); keymap.load(j.shortcuts)
  await refreshGit()                           // so worktrees/branches exist
  await _restoreLayout(j.layout)
catch any:
  try copy config.json → config.json.bak (overwrite)
```
**QUIRK**: state loaded before the exception stays in memory, and the next save writes
that partial state over the original (the `.bak` is the only safety net). Type errors
in `Workspace.fromJson`, `PaneGroup.fromJson`, `Launcher.fromJson` (e.g. `name: 5`)
throw (they cast with `as String?`) and abort the load this way; `Folder`/
`FeatureOrHotfix` entries are individually try/caught. Recommended Rust behavior: keep
the same `.bak` on a JSON parse failure, but treat any wrongly-typed entry as invalid
(skip) instead of aborting — strictly more robust, same result for valid files.

Typed-field reading rules (Dart casts): a present field with the wrong type is an error
for that entry. `as bool?` on a non-bool throws.

#### `_restoreLayout(layout)`

```
if layout == null: return
saved = layout.panes filtered to objects; if empty: return
_restoring = true
restored = []                     // one slot per saved pane, null when it didn't come back
try:
  for pane in saved:
    tab = _openPane(pane); restored.push(tab); if !tab: continue
    tab.hooks.touched   += pane.touched (strings only)
    tab.hooks.plans     += pane.plans.map(PlanNote.fromJson).nonNull
    tab.hooks.suggestions += pane.suggestions.map(SuggestedTask.fromJson).nonNull
    tab.done   = pane.done == true
    tab.pinned = pane.pinned == true
    if groups has id == pane.group: tab.groupId = pane.group
finally: _restoring = false
if no non-null restored: return
at(i) = restored[i] if 0<=i<len else null
if layout has "tree":         panes = Panes.fromJson(tree, i -> at(i)?.id)
elif !layout.containsKey("active"): panes = null            // screen was left empty
elif at(layout.active ?? 0) is left:                        // pre-grid legacy
    right = at(layout.split)
    panes = right ? Split(row,[Leaf(left),Leaf(right)],[.5,.5]) : Leaf(left)
focusedPaneId = at(layout.focused)?.id ?? first(order(panes))
notify      // no banner
```
Note: `_openPane` registers tabs (which places them in `panes` via `_place`); the final
assignment above overrides that placement. Saves are suppressed during restore.

#### `_openPane(pane)` — rebuild a tab from a recipe (used by restore and `openGroup`)

```
root   = pane.folderRoot ?? pane.projectRoot             // legacy key
folder = pane.loose == true ? loose : folders.find(root == f.root)
cwd    = pane.cwd
if !folder || !cwd || !isDir(cwd): return null
label  = pane.label
feat   = featureOrHotfixById(pane.featureOrHotfixId ?? pane.projectId)   // legacy key
match pane.kind:
  "reader":  doc = MxDoc.fromJson(pane.doc ?? {}); if !doc return null
             tab = _newReader(doc, folder, cwd, feat)
  "browser": return null                                  // removed feature; dropped on next save
  "setup":   setup = MxSetup.fromJson(pane.setup ?? {}); if !setup || !isDir(setup.root) return null
             tab = _newSetup(setup, folder, cwd, feat)
  "claude":  resumeId = pane.sessionId
             asleep = pane.hibernated == true && resumeId != null
             tab = openClaude(folder, cwd, label, feat, resumeId, start: !asleep)
  _ (any other, incl. "shell"):
             tab = openShell(folder, cwd, feat, launcher: launcherById(pane.launcher))
if label != null: tab.customLabel = label
tab.tint = MxTint.byName(pane.tint)
tab.zoom = Mx.type.clampZoom((pane.zoom as num)?.toInt() ?? 0)
       // clampZoom: clamp(z, ceil(9 - size), floor(24 - size)) → [-4, 11] at size 13
return tab
```

### 2.5 Pane JSON: recipe and layout entry

`recipe` (what a group stores), keys in order:

| key | when | value |
|---|---|---|
| `folderRoot` | always | `folder.root` |
| `loose` | folder is loose | `true` |
| `featureOrHotfixId` | non-null | id |
| `kind` | always | `shell`/`claude`/`reader`/`setup`/`plugin` |
| `cwd` | always | |
| `launcher` | launcher non-null | launcher **id only** |
| `label` | customLabel non-null (even `''`) | |
| `tint` | non-null | tint name |
| `sessionId` | `resumable` (claude with resumeId) | resumeId |
| `doc` | reader | `MxDoc.toJson` (xref reader): `{source, title, path?, origin?, at, text?}`; `text` only for non-file sources, clipped at 131072 UTF-16 units + `"\n\n…"` |
| `setup` | setup | `{root, selected?}` |
| `zoom` | ≠ 0 | int |

`MxTab.toJson` (layout entry) = recipe + (in order):

| key | when |
|---|---|
| `group` | groupId non-null |
| `touched` | non-empty (string list) |
| `plans` | non-empty (`PlanNote[]`) |
| `suggestions` | non-empty (`SuggestedTask[]`) |
| `done` | `true` when done |
| `hibernated` | `true` when hibernated |
| `pinned` | `true` when pinned |

Never persisted: followUps, armed, bornOf, restedAt, unseen, context, branch, dirty,
agentName, owner/ownerTag/embedded, onSessions, notices.

Example (layout_test): claude tab with label and session →
`{"folderRoot":"/repo","kind":"claude","cwd":"/repo/.claude/worktrees/TASK-47730","label":"TASK#47730","sessionId":"abc-123"}`.
A shell tab never writes `sessionId` even if one was set.

`DocSource` names (xref): `plan, file, message, report, plugin`; only `file` is on-disk
(re-read from `path`).

### 2.6 Pane tree JSON

```
Tree   = int                                  // leaf: index into the panes list
       | { "axis": "row"|"column", "weights": [num...], "children": [Tree...] }
```
`toJson(node, indexOf)`:
- leaf → `indexOf(tabId)`; if < 0 → null (dropped).
- split → serialize children, drop nulls (and their weights); none → null; one → that
  child directly; else normalize the kept weights to sum 1 (if sum ≤ 0 → equal) and
  emit `{axis, weights, children}`.

`fromJson(json, tabIdAt)`:
- `int` → `tabIdAt(i)` → leaf or null.
- not a map → null.
- map: for each child i, `fromJson`; skip nulls; weight = `weights[i]` as double if
  present else `1`; none → null; one → child; else normalize, axis = `column` iff
  `json.axis == "column"` else `row`.

### 2.7 Sidebar section & legacy migrations (`readSidebar`, public for tests)

```
legacy = (j.folders == null)
rawFolders = (legacy ? j.projects : j.folders) filtered to objects
for p in rawFolders: try folders.push(Folder.fromJson(p)) catch skip
if !legacy:
  for p in (j.featuresOrHotfixes ?? j.projects ?? []):     // `??` = only when key missing/null
    try featuresOrHotfixes.push(FeatureOrHotfix.fromJson(p)) catch skip
for w in j.workspaces ?? []: if Workspace.fromJson(w) is ws: workspaces.push(ws)
// 2.4.0 stamp: folders carried "workspace": "<path to .code-workspace>"
for p in rawFolders:
  path = p.workspace; if not non-empty string: continue
  w = workspaces.find(codeWorkspacePath == path)
      ?? push new Workspace(id: newWorkspaceId(), name: CodeWorkspace.nameOf(path),
                            codeWorkspacePath: path)
  root = p.root; if not string: continue
  if root ∉ w.folderRoots: w.folderRoots.push(root)
  if p.collapsed == true: w.collapsedFolders.add(root)
rootOrder = j.rootOrder filtered to strings
```

`Folder.fromJson`: `root` (String, required), `name` (String, required), `collapsed`
(bool?, default false), `tint` (name). Extra keys ignored (e.g. legacy
`worktreesCollapsed`, `workspace`). The legacy `workspace` key is **never written**.

`Workspace.fromJson(j)`: not a map → null. `name = trim(j.name ?? '')`,
`id = trim(j.id ?? '')`, `legacyPath = trim(j.path ?? '')`. Null if name empty, or id
and legacyPath both empty. `id` empty → `newWorkspaceId()`.
`codeWorkspacePath = j.codeWorkspacePath ?? (legacyPath empty ? null : legacyPath)`.
`folderRoots`/`collapsedFolders` = string elements only. `collapsed` default false.

`PaneGroup.fromJson(j)`: not a map → null; `id = j.id ?? ''`, `name = trim(j.name ?? '')`,
`panes = j.panes` filtered to maps; null if id empty, name empty, or panes empty.
`tree = j.tree` raw.

`Launcher.fromJson(j)`: not a map → null; `id = j.id ?? ''`, `name`/`command` trimmed;
null if any empty; icon by name.

`FeatureOrHotfix.fromJson`: id/folderRoot/name required Strings; brief default `''`;
collapsed default false; kind by name (default feature); tint by name.

Legacy summary (all must keep working):

| on disk | becomes |
|---|---|
| no `folders` key, `projects` = folder objects | folders (pre-projects format); features empty |
| `folders` + `projects` (features, pre-2.5) | `featuresOrHotfixes`, kind = feature |
| both `featuresOrHotfixes` and `projects` | only `featuresOrHotfixes` |
| `Folder.workspace` path stamp (2.4.0) | that folder added to the workspace with that `codeWorkspacePath` (created, named after file, if missing); `collapsed: true` → `collapsedFolders` |
| workspace `{path, name, collapsed}` without id (2.4.0) | new id, `codeWorkspacePath = path` |
| no `rootOrder` | `sidebarRows` derives the order (§3.4) |
| pane `projectRoot` | `folderRoot` |
| pane/recipe `projectId` | `featureOrHotfixId` (new key wins when both present) |
| pane `kind: "browser"` | dropped |
| layout without `tree`, with `active`/`split` ints | row split of two (or single leaf) |
| FeatureOrHotfix without `kind` | feature |
| unknown tint name | no tint |
| unknown theme id | retired map → `maestria` |

Fixture `test/fixtures/config-2.4.0.json` (must load as described in §5):
```json
{
  "folders": [
    {"root": "/repos/atrium-backend", "name": "atrium-backend", "collapsed": false, "workspace": "/repos/atrium.code-workspace"},
    {"root": "/repos/atrium-frontend", "name": "atrium-frontend", "collapsed": true, "workspace": "/repos/atrium.code-workspace"},
    {"root": "/repos/solta", "name": "solta", "collapsed": false},
    {"root": "/repos/uplii-ai", "name": "uplii-ai", "collapsed": false, "workspace": "/repos/uplii.code-workspace"}
  ],
  "workspaces": [
    {"path": "/repos/atrium.code-workspace", "name": "atrium", "collapsed": true}
  ],
  "projects": [
    {"id": "pj1", "folderRoot": "/repos/atrium-backend", "name": "permissão do google", "brief": "", "collapsed": false}
  ]
}
```

### 2.8 Full example of a current `config.json`

```json
{
  "folders": [{"root": "/home/u/repos/api", "name": "api", "collapsed": false, "tint": "cyan"}],
  "workspaces": [{"id": "wsm3abcd0", "name": "ATRIUM", "folderRoots": ["/home/u/repos/api"]}],
  "rootOrder": ["workspace:wsm3abcd0"],
  "featuresOrHotfixes": [{"id": "pjm3abce", "folderRoot": "/home/u/repos/api", "name": "login quebrado", "brief": "", "collapsed": false, "kind": "hotfix"}],
  "groups": [{"id": "gpm3abcf", "name": "grid da manhã", "panes": [{"folderRoot": "/home/u/repos/api", "kind": "shell", "cwd": "/home/u/repos/api"}]}],
  "launchers": [{"id": "lch1729500000000000", "name": "btop", "command": "btop", "icon": "monitor"}],
  "sidebarWidth": 352.0,
  "theme": "maestria",
  "layout": {
    "panes": [
      {"folderRoot": "/home/u/repos/api", "kind": "claude", "cwd": "/home/u/repos/api", "label": "TASK#1", "sessionId": "0b7c…", "touched": ["/home/u/repos/api/a.dart"], "done": true}
    ],
    "tree": 0,
    "focused": 0
  }
}
```
(A group with a single pane has no `tree` key.)

### 2.9 Chat history (read-only): `~/.claude/projects`

`ChatHistory.home = $HOME/.claude/projects` (`$HOME` empty → `/.claude/projects`). Under
test the store uses `test/fixtures/history` (make it injectable).

`dirFor(cwd) = cwd with every char not in [A-Za-z0-9] replaced by '-'` (lossy).

`read({root, cwds?, on?: date, limit = 40})`:
1. places: if `cwds == null`: every **directory** directly under root (cwd unknown); if root
   missing → `[]`. Else for each non-empty cwd: `root/dirFor(cwd)` with that cwd attached.
2. For each place, skipping duplicate dir paths and non-existing dirs: every **file**
   directly inside whose name ends with `.jsonl` (subdirs such as `<id>/subagents/`
   ignored); stat it.
3. Sort by mtime descending. If `on` given: keep files whose mtime is the same local
   calendar day. Take the first `limit`.
4. For each, build a `ChatEntry`:
   - `sessionId` = filename minus `.jsonl`.
   - Read the first **32 KiB** (32768 bytes), UTF-8 decode with malformed allowed; on I/O
     error use `''`.
   - Split by `\n`; for each line starting with `{` that parses as a JSON object:
     first non-empty string `cwd` → `inside`; by `type`: `ai-title` → first `aiTitle`
     (`titled`); `custom-title` → first `customTitle` (`named`); `agent-name` → first
     `agentName` (`named`, if still unset); `user` → first `said(event)` (`spoke`).
   - `said(event)`: null if `isSidechain == true`; `message.content` string → it; list →
     first block with `type=="text"` → its `text`.
   - `cwd = askedCwd ?? inside ?? ''`.
   - `title = clean(titled) ?? clean(spoke) ?? clean(named) ?? sessionId.split('-')[0]`.
   - `clean(text)`: for each line: `bare = trim`, `one = bare` minus leading `^#+\s*`;
     skip if empty or starts with `<`; push `one`; if `bare` does **not** start with `#`,
     stop. Join pushed parts with `' '`; empty → null; >72 → first 72 + `…`.
   - `at` = mtime, `bytes` = size, `missing = cwd.isEmpty || !isDir(cwd)`.

`ChatStanding` (`AppStore.standingOf(chat)`), in order:
- `onScreen` if any tab's `resumeId == chat.sessionId` (note `já está num painel`)
- `live` if `agents.latest` has a row with that sessionId (note `aberta fora daqui`)
- `gone` if `chat.missing || cwd empty || !isDir(cwd)` (note `a pasta não existe mais`)
- else `fresh` (note null). `opens = fresh`.

### 2.10 `.code-workspace` (read-only, JSONC)

`CodeWorkspace.ext = '.code-workspace'`. `looksLikeOne(p) = lower(trim(p)).endsWith(ext)`.

`read(rawPath)`: `path = expandHome(trim(rawPath))`; missing file → null; read error →
null; else `parse`.

`parse(source, path)`:
1. `stripJsonc`: (a) remove comments outside strings — `//` to end of line (newline
   kept) and `/* … */` (unterminated runs to EOF); (b) remove a `,` (outside strings)
   followed only by whitespace (space, tab, \n, \r) then `}` or `]`. Both passes track
   strings with `\` escapes.
2. JSON decode; failure or not an object → null.
3. `from = dirOf(path)` (`.` if no `/`; `/` if only leading slash).
4. For each `folders[]` entry that is an object with non-empty trimmed `path`:
   `path = resolve(raw, from)`; `name` = trimmed `name` or null if empty.
5. `name` of workspace = `nameOf(path)`: basename minus case-insensitive
   `.code-workspace`; if that is empty, the basename.

`resolve(raw, from)`: `e = expandHome(trim(raw))`; absolute if starts with `/`, else
`from + '/' + e`; then `normalize`: split by `/`, drop empty and `.`, `..` pops the
previous segment (unless previous is `..`); in a relative path an excess `..` is kept,
in an absolute one dropped. Absolute → `'/' + join`; relative empty → `'.'`.

`WorkspaceImport { workspace, added: Folder[], already: Folder[], missing: string[] }`,
`sticky = missing.nonEmpty`, `summary`:
parts (in order, omitted when empty):
`'<n> pasta adicionada'` / `'<n> pastas adicionadas'`;
`'<n> já estava aqui'` / `'<n> já estavam aqui'`;
`'<n> não existe no disco'` / `'<n> não existem no disco'`.
No parts → `'Workspace "<name>": nada pra adicionar'`; else
`'Workspace "<name>": ' + parts.join(', ')`.

### 2.11 `expandHome(path)`

Only `~` and `~/…` expand, using `$HOME`; if HOME unset/empty → unchanged. `~` → HOME;
`~/x` → HOME (without trailing `/`) + `/x`. `~other/…`, relative, absolute, empty, and a
`~` elsewhere (`notas.md~`) are unchanged.

---
## 3. Store operations

Format: **op(inputs)** → effects. Unless stated, every mutating op ends with *save +
notify*. "banner(t)" = `showBanner(t)` (6 s), "banner!(t)" = sticky banner.

### 3.1 Startup, timers, shutdown

**init()** (async):
1. Start hooks server (xref); subscribe `events → applyHook`, `statuses → applyStatus`;
   set hooks desk = store (suggestions). Subscribe agents updates → `applyAgents`.
   Plugin RPC handler; plugin listener → `_syncScreen` + notify.
2. `await _loadConfig()` (§2.4). If plugins not scanned yet → scan.
3. Plugin startup, land saved plugin screen, sync sidebar (xref plugins).
4. `agents.start()`.
5. Periodic timers:
   - **1 s**: `pumpFlows()`; `hibernateIdle()`; `seeFocused()`; notify if any tab has
     status `tool` or is `armed` (elapsed-seconds readouts).
   - **10 s**: `refreshGit()`.
   - **20 s**: notify if any tab is `rested` (age labels).
6. notify.

Wiring from the app shell (main.dart): window resume → `setWindowActive(true)`,
inactive → `setWindowActive(false)`, exit requested → `await shutdown()` then exit.

**shutdown() / dispose()**: §2.3.

**notifyListeners()**: no-op if `_gone`; sets
`agents.eager = tabs.any(kind==claude && !exited && resumeId == null)`; emits change.

### 3.2 Preferences

| op | guard | effect |
|---|---|---|
| `setTheme(p)` | same id → no-op | apply palette |
| `setTypography(t)` | equal → no-op | apply |
| `zoomFocused(by)` | no focused tab, or clamped equal → no-op | `tab.zoom = clampZoom(zoom+by)` |
| `resetZoomFocused()` | no tab or zoom 0 → no-op | zoom = 0 |
| `bindShortcut(action, chord, replacing?)` | — | keymap.bind; if it stole from another action → banner(`'<chord.label> saiu de "<stolen.label>"'`) |
| `unbindShortcut`, `resetShortcuts(action?)` | — | keymap ops |
| `setSidebarWidth(w)` | clamp [248,680]; equal → no-op (called every drag frame) | |
| `toggleSidebar()` / `setSidebarHidden(h)` | equal → no-op | + `_syncSidebar` (xref) |
| `setStatusBarTop(b)` | equal → no-op | |
| `setDoNotDisturb(on)` / `toggleDoNotDisturb` | equal → no-op | on → clear all toasts |
| `setSuggestionsToPlugin(on)` | equal → no-op | |
| `setHibernateMinutes(m)` | clamp [0,1440]; equal → no-op | |
| `toggleGroupsCollapsed()` | — | flip |
| `showSidebarView(pluginId?)`, `moveRailPlugin` | — | xref plugins |

### 3.3 Folders

**addFolder(rawPath)** → Folder?:
`outcome = _adoptFolder(rawPath)`; folder null → banner!(`'Pasta não encontrada: <expandHome(rawPath)>'`), return null.
Not created (already existed) → return it (no save). Created → save, `await refreshGit()`, notify.

**_adoptFolder(rawPath)** → (folder?, created): `path = expandHome(rawPath)`;
`root = Git.mainRoot(path) ?? path`; existing folder with that root → (it, false);
`!isDir(root)` → (null, false); else push `Folder(root, name = last segment of root)` →
(it, true). No save/notify.

**importWorkspace(rawPath)** → WorkspaceImport?:
1. `ws = CodeWorkspace.read(rawPath)`; null → banner!(`'Não consegui ler esse workspace: <expandHome(trim(rawPath))>'`), null.
2. `ws.folders` empty → banner!(`'O workspace "<ws.name>" não lista nenhuma pasta'`), null.
3. For each entry: `_adoptFolder(entry.path)`: null → `missing += entry.path`; not
   created → `already += folder`; created → if `entry.name != null && folder.root ==
   entry.path` then `folder.name = entry.name`; `added += folder`.
4. `adopted = added + already`; if non-empty: find workspace with
   `codeWorkspacePath == ws.path`, else `_pinRootOrder()`, create
   `Workspace(newWorkspaceId(), ws.name, codeWorkspacePath: ws.path)`, push, append
   `rowKey(w)` to rootOrder. Append each adopted root not already in `w.folderRoots`.
5. save; if added non-empty → `await refreshGit()`; banner(summary, sticky = missing
   non-empty); notify; return result.
(No adopted folder → no workspace is created.)

**setFolderTint(f, tint?)**, **renameFolder(f, name)** (no trim/validation),
**toggleCollapsed(f)**.

**removeFolder(f)** (async): remove from `folders`; remove its root from every
workspace's `folderRoots` and `collapsedFolders`; remove `folder:<root>` from rootOrder;
remove all features with that folderRoot; `closeTab` every tab with that folderRoot.
(A workspace left empty stays.)

**focusedFolder**: focused tab's folder (loose → loose; else folder by root); else
`folders.first ?? loose`.

### 3.4 Workspaces and root order

Queries:
- `foldersOf(w)` = w.folderRoots mapped to existing folders (skip missing), in order.
- `workspacesOf(f)` = workspaces containing f.root, in workspaces-list order.
- `standsAlone(f)` = in no workspace.
- `workspaceById(id)`.
- `rowKey(row)` = `'workspace:<id>'` | `'folder:<root>'`.
- `_rowOf(key)`: `workspace:` → workspace by id; `folder:` → folder by root **only if it
  stands alone**; else null.
- **sidebarRows** (derived root order):
  ```
  rows=[], seen={}
  add(r): if seen.insert(rowKey(r)): rows.push(r)
  for key in rootOrder: if _rowOf(key) is row: add(row)
  for f in folders: ws = workspacesOf(f); if ws empty: add(f) else ws.forEach(add)
  workspaces.forEach(add)
  ```
  (Legacy/no rootOrder: a workspace appears where its first folder was; unknown keys skipped;
  new rows append at the end.)
- `_pinRootOrder()`: `rootOrder = sidebarRows.map(rowKey)`. Every root mutation starts
  with it.

Ops:
- **createWorkspace(name, folders=[], codeWorkspacePath?)**: `label = trim(name)`; empty →
  **throw ArgumentError** (`'não pode ser vazio'`); `_pinRootOrder`; create with
  `newWorkspaceId()`; push; append its key to rootOrder; add folder roots (dedup); save,
  notify; return it.
- **addToWorkspace(f, w, before?)**: already in → no-op. `_pinRootOrder`; insert at index
  of `before.root` in w.folderRoots if found, else append.
- **removeFromWorkspace(f, w, at?: SidebarRow)**: not in → no-op. `_pinRootOrder`;
  remove root from folderRoots and collapsedFolders; if f now stands alone: insert
  `rowKey(f)` into rootOrder at `indexOf(rowKey(at))` if `at` given and found, else right
  after the workspace (`indexOf(rowKey(w)) + 1`).
- **renameWorkspace(w, name)**: trimmed; empty or unchanged → no-op.
- **setWorkspaceTint(w, tint?)**.
- **linkCodeWorkspace(w, path?)**: `clean = trim(path)`; null/empty → null; else
  `expandHome(clean)`. Does not read the file.
- **dissolveWorkspace(w)**: `_pinRootOrder`; `at = indexOf(rowKey(w))`; remove its key and
  the workspace; `freed` = keys of `foldersOf(w)` that now stand alone; insert them at
  `at` (or end if at<0). No tab closes.
- **closingWith(w)** = `foldersOf(w)` that are in exactly 1 workspace.
- **closeWorkspace(w)** (async): `going = closingWith(w)`; `await removeFolder` each;
  `_pinRootOrder`; remove key and workspace; save, notify; banner
  `'Workspace "<name>" fechado — 1 pasta saiu da lateral'` if exactly 1, else
  `'Workspace "<name>" fechado — <n> pastas saíram da lateral'`.
- **toggleWorkspaceCollapsed(w)**.
- **isFolderCollapsed(f, within?)** = within null ? f.collapsed : within.collapsedFolders ∋ f.root.
- **toggleFolderCollapsed(f, within?)**: within null → toggleCollapsed(f); else toggle
  membership in within.collapsedFolders.
- **_slide(list, from, to)**: false if `from<0 || to<0 || from==to`; else
  `list.insert(to, list.removeAt(from))` → true. (Result: dropped item occupies the slot
  of the target — after it when coming from above, before it when coming from below.)
- **moveRootRow(row, target)**: `_pinRootOrder`; `_slide(rootOrder, idx(row), idx(target))`; if moved → save+notify.
- **moveInWorkspace(w, f, target)**: `_slide(w.folderRoots, …)`.
- **moveFolder(f, from?, to, before?)**: `addToWorkspace(f, to, before)`; if `from != null &&
  from != to` → `removeFromWorkspace(f, from)`.
- **canDrop(from: RowPlace, onto: RowPlace, into=false)** (RowPlace = (row, within?)):
  - into: `from.row is Folder && onto.row is Workspace && from.within != onto.row`.
  - same row identity and same `within` → false.
  - from.row is Workspace → `onto.within == null`.
  - else true.
- **drop(from, onto, into=false)**: if !canDrop → no-op.
  - into → `moveFolder(folder, from: from.within, to: onto.row)`.
  - row is Workspace → `moveRootRow(row, onto.row)`.
  - folder, `onto.within == null`: from.within null → `moveRootRow(f, onto.row)`; else
    `removeFromWorkspace(f, from.within, at: onto.row)`.
  - folder, onto inside lane L: `from.within == L` → `moveInWorkspace(L, f, onto.row)`;
    else `moveFolder(f, from: from.within, to: L, before: onto.row)`.
- **hasHitsInWorkspace(w)** = any folder of w `hasHits`.

### 3.5 Features / hotfixes

- **featuresOrHotfixesOf(f)** (by folderRoot, works for loose), **featureOrHotfixById**,
  **featureOrHotfixOf(tab)**, **focusedFeatureOrHotfix** (of focused tab).
- **addFeatureOrHotfix(f, name, brief='', kind=feature)**: id `pj…`; no trim on name.
- **editFeatureOrHotfix(x, name?, brief?)**: name applied trimmed only if non-blank;
  brief applied as-is if non-null. Always save.
- **setFeatureOrHotfixTint(x, tint?)**; **setFeatureOrHotfixKind(x, kind)** (equal → no-op);
  **toggleFeatureOrHotfixCollapsed(x)**.
- **completeFeatureOrHotfix(x)** → int: closeTab every tab in it; remove it; return count.
- **removeFeatureOrHotfix(x)** ("dissolve"): remove; its tabs get `featureOrHotfixId = null`
  (stay open).
- **assign(tab, x?)**: if x non-null and `x.folderRoot != tab.folderRoot` → no-op; if
  already equal → no-op; else set.
- **tabsIn(x)**; **needingHumanIn(x)** = tabsIn where `!done && status.needsHuman`.

### 3.6 Opening tabs

**openShell(f, cwd?, command?, featureOrHotfix?, launcher?, place=true)** → tab:
new MxTab(kind shell, cwd = cwd ?? f.root, branch '', launcher); feature attached only
if `feature.folderRoot == f.root`; `_register(tab, place)`; `run = command ??
launcher?.command`; none → `term.startShell(cwd)`; else `term.startCommand(run, cwd)`.

**openLauncher(launcher, f, cwd?, feature?)** = openShell with launcher.

**openClaude(f, cwd, label?, resumeId?, featureOrHotfix?, prompt?, start=true)** → tab:
new MxTab(kind claude, customLabel = label); `sessionId = resumeId`; feature attached if
same folder; `_register(tab)`; `agentName = (label null/empty) ? last segment of cwd :
label`; if start → `_launchClaude(tab, resumeId, prompt)`; else hibernated-at-birth:
`hibernated = true`, `term.park()` (exited=true, no process), `hooks.status = ended`,
`term.remark('Hibernada — a conversa volta com um clique')`. Then async
`Git.branchOf(cwd)` → `tab.branch = b`, notify.

**_launchClaude(tab, resumeId?, prompt?)**: see §4.4 for the exact command; then a **3 s**
timer: if tab still in tabs and not exited and `hooks.settle()` → notify.

**_register(tab, place=true)**: push to tabs; plugin event `session.opened`
`{tabId, kind, cwd, folder: folder.root}` (activation `onSession`); if place → `_place(tab)`;
set `term.onExit`:
- if tab no longer in tabs → ignore;
- if kind claude and alive < **5 s** (`now - startedAt`) → banner!(`earlyExitMessage(title, exitCode)`);
- plugin event `session.exited {tabId, code}`; save; notify.
Then save, notify.

`earlyExitMessage(title, code)`:
- 127 → `'<title>: não achei o `claude` no PATH do app — instale-o, ou ponha a pasta do binário no <Sh.profileFile> (o rc do shell interativo o app não lê)'`
- else → `'<title>: o Claude saiu na largada (código <code or ?>) — abra o painel pra ver o motivo'`
(The backticks around `claude` are literal characters in the message.)

`_newReader(doc, folder, cwd?, feature?)` / `_newSetup(...)`: create tab (cwd ?? folder.root),
attach feature if same folder, **push to tabs without `_register`** (no process, no event,
no placement).

**relaunch(tab)**: only if `tab.launcher != null && tab.exited` → `term.relaunch(command, cwd)`; notify (no save).

### 3.7 Placing, focusing, closing (pane tree management)

Screens: `screenOf(tab)` = null (sessions) unless the tab is a plugin view / owned
terminal of a plugin that has its own active screen and `!onSessions` (xref plugins).
With no plugins, everything is on the sessions screen and `_enter` is a no-op. `_enter(s)`
parks the current (tree, focus), restores the other's, and clears `_hole`.

- `openPanes` = tabs for `order(panes)` that still exist; `paneCount`; `isOpen(tab) = has(panes, tab.id)`;
  `_placed(tab) = has(treeOf(screenOf(tab)), tab.id)`.
- `focusedTab = byId(focusedPaneId) ?? byId(first(order(panes)))`.

**select(tab)** (sidebar click):
1. embedded → return.
2. hibernated → `wake(tab)` (async, not awaited).
3. `_enter(screenOf(tab))`.
4. `group = activeGroup`; if `group != null && tab.groupId != group.id && no open pane is
   pinned` → `panes = Leaf(tab)`, focus = tab (replaces the whole grid).
5. else `_place(tab)`.
6. save, notify.

**_place(tab)** (no save/notify):
```
_enter(screenOf(tab))
if panes == null: panes = Leaf(tab.id)
elif !has(panes, tab.id):
  focused = focusedTab
  keeps(t) = _screen != null && t.isPluginView && !tab.isPluginView     // false on sessions screen
  loose = (focused && !isPinned(focused) && !keeps(focused)) ? focused.id
          : first openPane t with !isPinned(t) && !keeps(t) ?.id
  if loose == null && focused && keeps(focused) && !isPinned(focused):
      panes = insert(panes, tab, target: focused, right)
  elif loose != null: swap(panes, loose, tab.id)        // replace content of that leaf
  elif !_fillHole(tab.id): panes = edge(panes, tab.id, right)
focusedPaneId = tab.id
```

**_hole**: when a non-pinned on-screen tab leaves the tree (`_drop`) while at least one
remaining open pane is pinned, remember `(copy of tree before removal, removed id)`.
`_fillHole(id)`: take and clear `_hole`; if none → false; if
`!sameShape(remove(copy(hole.tree), hole.id), panes)` → false; else
`swap(hole.tree, hole.id, id)`, `panes = hole.tree` (restores exact old weights) → true.

**_placeBeside(tab, beside?)**: `_enter(screenOf(tab))`; if beside is pinned → beside =
first unpinned open pane; if `panes == null || beside == null || !has(panes, beside)` →
`_place(tab)`; else `panes = insert(panes, tab, target: beside, right)`, focus = tab.

**_drop(tab)** (remove from tree, keep tab):
1. For every parked screen tree containing tab: remove it; clear that parked focus if it was the tab.
2. `loose = has(panes, tab) && !isPinned(tab)`; `tree = loose ? copy(panes) : null`.
3. `before = order(panes)`, `at = before.indexOf(tab.id)`; `panes = remove(panes, tab.id)`.
4. If tree && any open pane is pinned → `_hole = (tree, tab.id)`.
5. If focus was this tab: `left = order(panes)`; focus = `left.isEmpty ? null :
   left[clamp(at, 0, len-1)]`.

**dismiss(tab)** (header ✕ — remove from screen, keep session): if not in panes → no-op;
`_drop`; save; notify.

**closeTab(tab)** (kill + remove):
1. If plugin view: close embedded terminals referenced by `terminal` blocks (recursively
   through `children`) whose `embedded && owner == view.pluginId`.
2. `armed = false`. If `!isPassive` → plugin event `session.closed {tabId}`.
3. `term.kill()` (not awaited: SIGHUP to process group, wait ≤3 s, then SIGKILL).
4. Remove from tabs; `_drop(tab)`.
5. If `panes == null`: last remaining tab with `!embedded && screenOf(t) == _screen` →
   `_place(it)`.
6. save, notify. (FollowUps list is left untouched on the dead object.)

**closeFocused()**: closeTab(focusedTab) if any.

**closeSettled(f)** → int: closeTab every `tabsOf(f)` with `done`; return count. Exited
tabs are **not** swept. **settledIn(f)** = count of done in tabsOf(f).
UI banners after sweeping (sidebar): 0 → `'Nada pra limpar aqui — nenhum painel marcado
como concluído'`; 1 → `'Um painel fechado — o que estava marcado como concluído'`; n →
`'<n> painéis fechados — os que estavam marcados como concluídos'`.

**renameTab(tab, label)** (no trim); **setTabTint(tab, tint?)**.

**setDone(tab, done)** → int dropped: equal → 0 (no save). Set; if true: `armed = false`,
dropped = followUps.length, clear followUps. save, notify.

**moveTab(tab, target)**: `from/to` indexes in tabs; any <0 or equal → no-op;
`removeAt(from)`, `insert(to, tab)`; `tab.featureOrHotfixId = target.featureOrHotfixId`.

**togglePin(tab)**: only if `isOpen(tab)`; flip `pinned`. **isPinned(tab)** = pinned && isOpen.

**dropTab(tab, target, side)** (drag onto a pane):
- `tab.id == target.id || !has(panes, target)` → `focusPane(tab)`; return.
- `screenOf(tab) != _screen` → `select(tab)`; return.
- `_drop(tab)`; center → if target is pinned, `tab.pinned = true`; `swap(panes, target, tab)`;
  else `panes = insert(panes, tab, target, side)`. focus = tab. save, notify.
  (On the next save the displaced pinned target loses its pin because it's off-screen.)

**focusPane(tab)**: if already focused or not on screen → no-op; set focus; notify (no save).

**cyclePane(delta)**: order of screen; <2 → no-op; index of focus (or first); wrap modulo;
notify (no save).

**cycle(delta)**: ring = tabs on current screen (in tabs order); current = index of
focusedTab; next = (current+delta) mod len (current=-1 handled by mod); `select(ring[next])`.

**selectIndex(i)**: valid → `select(tabs[i])`.

**resizeSplit(split, gutter, delta, total)**: `Panes.resize`; save; notify.

**watching(tab)** = `windowActive && focusedPaneId == tab.id && has(panes, tab.id)`.

**tabsOf(f)** = tabs with folderRoot == f.root, `!isPluginView`, and (`owner == null` or
owner plugin no longer installed). **needingHuman(f)** = tabsOf where `!done &&
needsHuman`. **waitingSessions** = claude tabs, `!done && needsHuman`.
**tabAt(path)** = first tab with `cwd == path && !exited`.

**revealTab(tab)**: `sidebarHidden=false`, `sidebarView=null`; if filtering and tab doesn't
match → clearSearch; `within` = first workspace in `sidebarRows` containing tab.folderRoot
(null for loose); within → `within.collapsed=false`, remove root from its collapsedFolders;
else (non-loose) `folder.collapsed=false`; feature.collapsed = false; `revealedTabId`,
`revealedWithin`, scroll-pending=true; 2 s timer clears them (if still the same tab, and
not `_gone`) + notify. save, sync sidebar, notify. `takeRevealScroll(tab, within)` → true
once for the matching appearance.

### 3.8 Readers & setup panes (store side; rendering is xref reader/setup)

- **showDoc(doc, from?, folder?, cwd?, feature?)**: source = from ?? focusedTab. Reuse
  the existing reader: first reader among openPanes, else any reader tab. If found:
  `reading.doc.become(doc)`, `customLabel = null`, if not on screen `_placeBeside(it,
  source)`, focus it, save, notify, return. Else create `_newReader(doc, folder: folder ??
  source?.folder ?? focusedFolder, cwd: folder != null ? cwd : source?.cwd, feature:
  folder != null ? feature : featureOf(source))`, `_placeBeside(tab, source)`, save, notify.
- **showPlan(tab, note?)**: plan = note ?? hooks.plan; none → banner(`'<title> não
  apresentou nenhum plano ainda'`), null. `which = plans.indexOf(plan)`; title prefix
  `'Plano <which+1>/<len>'` if `0 <= which < len-1` else `'Plano'`; showDoc(MxDoc(source
  plan, title `'<prefix> de <tab.title>'`, text, origin tab.title, at plan.at), from tab).
- **showMessage(tab)**: `lastMessageFull` blank → banner(`'<title> ainda não disse nada ao
  terminar um turno'`), null; else showDoc(source message, title `'Recado de <title>'`).
- **showFile(path, from?, folder?, cwd?, feature?)**: missing → banner!(`'Esse arquivo não
  está mais lá: <path>'`), null; else showDoc(MxDoc.file(path, origin: from?.title), …).
- **followLink(href, from?, base?)**: blank → return; scheme http/https/mailto → open via
  system (`Notifier.openLink`), failure → banner!(`'Não consegui abrir <href>'`); else path
  = part before `#`; empty → return; `target = resolveLinkPath(path, base ?? from.cwd)`;
  missing → banner!(`'Esse link aponta pra um arquivo que não existe: <target>'`);
  markdown → showFile; else Quick Look (xref).
- **openMarkdown(folder?, cwd?, feature?)**: native picker (xref) starting in
  `cwd ?? folder.root ?? focused.cwd ?? focusedFolder.root`; picked → showFile.
- **showSetup(folder, cwd?, feature?)**: one per root (`cwd ?? folder.root`): existing →
  place beside focused if off-screen, focus, save; else `_newSetup(MxSetup(root))`,
  `_placeBeside(tab, focusedTab)`, save.
- `reading` = doc of first reader on screen. `readable(path)` = isMarkdownPath (xref links).

### 3.9 Groups

- **saveGroup(name)** → group?: `title = trim(name)`; `open = openPanes`; null if title
  empty, open empty, or `_screen != null`. `at` = index of group whose name equals title
  **case-insensitively**. New group `id = at<0 ? 'gp'+base36(µs) : groups[at].id`,
  `name = title`, `panes = open.map(recipe)`, `tree = Panes.toJson(panes, id →
  open.indexOf)`. Append or replace at `at`. `_stamp(group, open)`. save, notify.
- **groupPanes()** → group?: open.length < 2 or plugin screen → null. `base =
  summarize(recipes)`; `name = base`; for n = 2.. while some group name equals name
  (case-insensitive): `name = '<base> (<n>)'`. Return `saveGroup(name)`.
- **_stamp(group, members)**: tabs with this groupId not in members → groupId = null;
  members → groupId = group.id.
- **ungroup(g)**: `_stamp(g, [])`; remove g.
- **removeGroup(g)**: remove g only (stamps left; they are ignored on restore and
  `groupOf` returns null for unknown ids).
- **clearGroups()** → count: 0 → 0; else stamp-clear all, clear list.
  UI banners: one group → `'Grupo "<name>" esquecido — os painéis continuam abertos'` (no
  confirmation); several → confirmation `'Esquecer os <n> grupos?'` then `'<n> grupos
  esquecidos — nenhum painel foi fechado'`.
- **renameGroup(g, name)**: trimmed; empty/unchanged → no-op.
- **groupById**, **groupOf(tab)**, **activeGroup**, **showing(g)**.
- **openGroup(g)**:
  ```
  _enter(null); _restoring = true; taken = {}; panels = []
  try: for pane in g.panes:
         tab = _adopt(pane, taken) ?? _openPane(pane); panels.push(tab); if tab: taken.add(tab.id)
  finally: _restoring = false
  first = first non-null; none → banner!('O grupo "<name>" não tem mais nenhum painel pra abrir'); return
  panes = Panes.fromJson(g.tree, i -> panels[i]?.id) ?? Leaf(first)
  focus = first(order(panes))
  for t in openPanes: if t.hibernated: wake(t)
  _stamp(g, panels.nonNull); save; notify
  ```
- **_adopt(pane, taken)** — reuse a live tab instead of spawning:
  - reader: any reader tab not taken; `doc = MxDoc.fromJson(pane.doc)`; both required;
    `reader.doc.become(doc)`, clear label.
  - setup: existing setup tab with same root not taken; `setup.become(...)`.
  - else first tab not taken with `kind.name == pane.kind`, `folderRoot == (pane.loose ?
    loose.root : pane.folderRoot)`, `cwd == pane.cwd`, `!exited`, and (`pane.sessionId ==
    null` or `t.resumeId == pane.sessionId`).

### 3.10 Launchers

- **launcherById(id)**.
- **addLauncher(name, command, icon=terminal)**: id `'lch' + µs`; name/command trimmed.
- **editLauncher(l, name?, command?, icon?)**: trimmed non-empty values applied.
- **removeLauncher(l)**: remove; tabs using it → `launcher = null` (stay open).
- **relaunch(tab)**: §3.6.

### 3.11 Hibernation

- `defaultHibernateMinutes = 30`; `autoHibernate = hibernateMinutes > 0`.
- **hibernate(tab)**: `!canHibernate` → no-op. Set `hibernated = true`, `armed = false`,
  `restedAt = null`, `unseen = false`, `hooks.status = ended`, `activeTool = null`,
  `toolStartedAt = null`; `term.remark('Hibernada — processo desligado pra liberar
  memória; a conversa volta com um clique')`; if `term.pid == null` → `term.park()` else
  `term.kill()` (not awaited). save, notify.
- **wake(tab)** (async): not hibernated → no-op. `hibernated = false`, status `starting`,
  notify; if `!exited` → `await term.kill()`; if tab no longer in tabs → return;
  `term.terminal.write('\r\n')`; `_launchClaude(tab, resumeId: tab.resumeId)`; save; notify.
- **hibernateIdle(now?)** (1 s tick): if !autoHibernate return. For each tab (snapshot):
  skip if `!canHibernate || _placed(tab)`; skip unless status is `ready` or atRest; skip if
  `armed || followUps nonEmpty || hooks.busyForks`; skip if any tab has a followUp with
  `targetTabId == tab.id`; `since = restedAt ?? hooks.lastEventAt ?? startedAt`; if
  `now - since >= hibernateMinutes` → hibernate. notify if any changed.

### 3.12 Git, worktrees, editor

- **refreshGit()**: for each folder (snapshot): `list = Git.worktrees(root)`; null → keep
  previous (continue); else `worktrees[root] = list`, `isRepo = list.nonEmpty`, `branch =
  main entry's branch or ''`. Then for each tab (snapshot, skip removed):
  `dirty = Git.dirtyCount(cwd) ?? dirty`; if `branch` empty → `branch = Git.branchOf(cwd)`.
  notify (no save).
- **newTask(p, taskId, branchPattern, dirPattern, baseRef?, setupCommand?, feature?, prompt?)**:
  `base = baseRef ?? Git.defaultRemoteRef(root) ?? 'origin/master'`;
  `branch = branchPattern.replaceAll('{id}', taskId)`;
  `dir = dirPattern.replaceAll('{id}', taskId).replaceAll('#', '-')`;
  `outcome = Git.addWorktree(root, dir, branch, base)`; banner(outcome.message, sticky:
  !ok); !ok → null. If setupCommand non-blank → `openShell(p, cwd: outcome.path, command:
  trim(setup), feature)` with `customLabel = '<taskId> setup'`. `await refreshGit()`;
  return `openClaude(p, cwd: outcome.path, label: taskId, feature, prompt)`.
- **removeWorktree(p, w, force=false, forceBranch?, deleteBranch=false)**: if `tabAt(w.path)`
  → banner!(`'Feche o painel que está nessa worktree antes de excluí-la'`), return.
  `Git.removeWorktree(...)`; banner(message, sticky: !ok); refreshGit.
- **pruneWorktrees(p)**: `Git.prune(root)`; banner; refreshGit.
- **openInEditor(path)**: neither dir nor file exists → banner!(`looksLikeOne(path) ? 'Esse
  workspace não está mais lá: <path>' : 'Essa pasta não existe mais: <path>'`); else
  `ok = Editor.open(path)`; banner(ok ? `'Aberto no vscode: <basename>'` : `'Não achei o
  vscode — nem o `code` no PATH, nem o app'`, sticky: !ok).

### 3.13 History

- **chatHome**: test → `test/fixtures/history`; else `ChatHistory.home`.
- **chatsIn(folder)**: loose → `allChats()`; else `ChatHistory.read(root: chatHome, cwds:
  [folder.root, ...worktrees[folder.root].paths])`.
- **allChats()** = `ChatHistory.read(root: chatHome)`.
- **folderAt(cwd)**: empty → null. `inside(root) = cwd == root || cwd startsWith root+'/'`.
  First folder whose root contains cwd; else first folder having a worktree path that
  contains cwd; else null.
- **standingOf(chat)**: §2.9.
- **resumeChat(chat, folder?, feature?)**: by standing:
  - onScreen → the tab with that resumeId; `select` it; return it.
  - live → banner!(`'"<chat.label>" ainda está rodando fora do MaestrIA — o Claude só
    retoma uma conversa depois que ela sai'`), null.
  - gone → banner!(cwd empty ? `'Não sei em que pasta "<label>" rodou'` : `'A pasta dessa
    conversa não existe mais: <cwd>'`), null.
  - fresh → `openClaude(folder ?? folderAt(chat.cwd) ?? loose, cwd: chat.cwd, label:
    chat.label, resumeId: chat.sessionId, feature)`.

### 3.14 Search and filters (never persisted)

- `setQuery(q)`: equal → no-op; `query = q`; `terms = lower(q).split(' ')` non-empty; notify.
- `toggleFilter(f)`, `toggleFilterFolder(folder)` (by root), `toggleFilterFeatureOrHotfix(x)`
  (by id): flip membership; notify.
- `filtering = terms ∨ filterRoots ∨ filterFeatures ∨ filterFlags` non-empty;
  `activeFilters = |roots| + |features| + |flags|`.
- `clearSearch()`: if !filtering and query empty → no-op; reset all; notify.
- `haystack(t) = lower(join(' ', [t.title, customLabel ?? '', folder.name, feature?.name ??
  '', branch, cwd, agentName ?? '', kind.name]))` (subtitle deliberately excluded).
- `matches(t)`: every term ⊂ haystack (substring); if roots or features filters set: tab
  must have root ∈ roots **or** featureId ∈ features; for each filter group with picks: at
  least one pick matches (OR within group, AND across groups).
- `visible(list)`, `hasHits(f) = tabsOf(f).any(matches)`, `hits = tabs.where(matches)`.

### 3.15 Banner, notices (bell), toasts

- **showBanner(text, sticky=false)**: set banner; cancel timer; non-sticky → 6 s timer →
  `clearBanner`. notify. **clearBanner()**.
- **_notice(tab, kind, suggestionId?, detail?)**: if kind ≠ suggested →
  `readNoticesOf(tab.id, notify: false)` (retire older notices of that tab); create notice
  (`title = tab.title`, `read = watching(tab)`); insert at 0; if > 50 → drop overflow (and
  their toasts); if `!read && !doNotDisturb && !noticesOpen` → `_toast`.
- **_toast(n)**: append; while > 3 drop first; `_armToast(n)`.
- **_armToast(n)**: cancel existing timer; if !windowActive → no timer; else 8 s →
  `dismissToast(n)`.
- **dismissToast(n)**, **holdToast(n, hold)** (hold → cancel timer; release → re-arm).
- **readNoticesOf(tabId, notify=true, suggestions=true)** → bool: mark unread notices of
  that tab read (skipping `suggested` when suggestions=false), dropping their toasts.
- **toggleNotices()**: flip; sessionsOpen=false; opening clears toasts. **closeNotices()**.
- **toggleSessions()**: flip; opening closes notices. **closeSessions()**. **openSession(tab)**:
  close list + `select`.
- **openNotice(n)**: drop its toast; tab gone → mark read, notify; else readNoticesOf(tab),
  close list, `select(tab)`.
- **dismissNotice(n)**, **readAllNotices()** (+ clear toasts), **clearNotices()**.
- `unreadNotices` count.

### 3.16 Window activity and "seen"

- **setWindowActive(active)**: equal → no-op. Inactive: `_awaySince = now`, cancel all
  toast timers, notify. Active: re-arm every toast; `landed` = tabs with `unseen` and
  `restedAt > _awaySince`; clear `_awaySince`; `quiet = doNotDisturb || toasts nonEmpty`;
  if !quiet: 1 → banner(`'<title> terminou enquanto você estava fora'`); >1 →
  banner(`'<n> painéis terminaram enquanto você estava fora'`). notify.
- **seeFocused()** → bool (1 s tick): tab = byId(focus); not watching → false; if
  `readNoticesOf(tab.id)` (notifying) and `!unseen` → true; if `!unseen` → false;
  `unseen = false`, notify, true.

### 3.17 Ingesting hooks, statusline, agents

**applyStatus(e: {tabId, payload})**: unknown tab → ignore; `now =
ContextUsage.fromStatusLine(payload)`; if percent, tokens, window all equal to before
(null-safe) → return; set; notify.

**applyHook(e: {tabId, name, payload})**:
1. Unknown tab → ignore. `before = hooks.status`; `producedBefore = touched.length`.
2. `HookReducer.apply(hooks, name, payload)` (§1.9).
3. Plugin event `hook {tabId, name, payload}` (activations `onHook:<name>`, `onHook:*`,
   requires hooks permission). If status changed → `session.status {tabId, status,
   previous}` (activation `onSession`).
4. If `done && name == 'UserPromptSubmit'` → `done = false`; save.
5. If `hooks.sessionId != null && tab.sessionId != hooks.sessionId` → copy; save.
6. If touched length changed → save.
7. If status changed: → waitingAnswer: `_notice(question)`; → waitingPermission:
   `_notice(permission)`; else if `!before.atRest && after.atRest`: `_notice(finished)`;
   else if `!after.needsHuman && !after.atRest`: `readNoticesOf(tab, notify: false,
   suggestions: false)`.
8. Rest edge (§1.16): arm queue, `restedAt`, `unseen = !watching(tab)`; or the reverse.
9. `_checkAlerts()`; notify.

**applyAgents(list)**: map our tabs by `term.pid`; for each row with a pid of ours: if
`name` non-empty → `agentName = name`; if `sessionId` non-null → `tab.sessionId =
sessionId`. Then `_checkAlerts()`. (No save, no notify — **QUIRK**: the session id is
saved on the next save triggered by something else; the Rust port may save here.)

**_checkAlerts()**: `waiting` = for every tab `!done && needsHuman`: key `'tab:<id>'` →
`'<title> · <callToAction>'`. For each key newly added to `_alerted` →
`notifier.alert('MaestrIA', text)` (desktop notification, xref). Remove from `_alerted`
keys no longer waiting. `notifier.badge(waiting empty ? null : '<count>')` (dock badge).

### 3.18 Flows (follow-up queues)

- **queue(tab, steps, now=false)**: replace followUps; `armed = steps.nonEmpty && (now ||
  armed)`; notify (no save).
- **startFlow(folder, prompt, steps, cwd?, label?, feature?)**: `openClaude(folder, cwd ??
  root, label, feature, prompt)`; `queue(tab, steps)`; banner(steps empty ?
  `'<title>: sessão aberta'` : `'<title>: fluxo de <n> passo armado'` /
  `'<title>: fluxo de <n> passos armado'`).
- **holdFor(tab, at?)**: `exited || !status.atRest` → working; `silence = last==null ?
  flowPatience : now - lastEventAt`; `silence >= flowPatience (3 min)` → go; `busyForks`
  → forks; `silence >= flowQuiet (5 s)` → go; else quiet.
- **pumpFlows()**: for each tab (snapshot): skip unless `armed && !exited &&
  followUps.nonEmpty`; if `holdFor == go` → `advance`.
- **advance(tab)**: exited or empty → return; pop first step; `armed = !handsBack &&
  followUps.nonEmpty`; `late = busyForks`; if late → clear forkIds, forksOut = 0;
  `runFollowUp(step, from: tab, late)` (not awaited); notify.
- **runFollowUp(step, from, late)**: from not in tabs → return. If late →
  banner!(`'<title>: os agentes dela não avisaram que terminaram — o fluxo seguiu assim
  mesmo'`). Then:
  - keepGoing: exited → return; `await term.submit(text)`; banner(`'<title>: próximo
    passo enviado'`).
  - newSession: `openClaude(from.folder, cwd: from.cwd, label: '<from.title> ▸ depois',
    feature, prompt: text)`; `_descend(new, from)`; banner(`'<from.title> terminou — abri
    <new.title> em seguida'`).
  - command: `openShell(from.folder, cwd: from.cwd, command: text, feature)`;
    `customLabel = shellLabel(text)`; `_descend`; banner(`'<from.title> terminou — rodando
    <text>'`).
  - handoff: target = byId(targetTabId); missing or exited → banner!(`'<from.title>
    terminou, mas o painel que ia receber não está mais aberto'`), return; `await
    target.term.submit(handoffText(from, text))`; banner(`'<from.title> passou a bola pra
    <target.title>'`).
- **shellLabel(cmd)**: collapse whitespace runs to one space, trim; ≤28 → it; else
  `trimRight(first 27) + '…'`.
- **handoffText(from, note)**: `'[MaestrIA] o painel "<from.title>" acabou de terminar.'`;
  if clipped lastMessageFull non-empty: + `'\n\nO que ele disse ao terminar:\n"""\n<said>\n"""'`;
  if note non-blank: + `'\n\n<trim(note)>'`. Clip: > 4000 → `'…' + last 4000`.
- **descendsFrom(tab, root)**: walk `bornOf` up to depth 8.
- **_descend(child, parent)**: `child.bornOf = parent.id`; move child in tabs to right after
  parent's existing descendants (contiguous run after parent that descend from it); save;
  notify.
- `term.submit(text)` (xref PTY): no-op if exited or blank; paste; wait 250 ms; if still
  alive send Enter.

### 3.19 Suggested tasks (store side of the task MCP, xref task_mcp)

- **suggest(tabId, title, tldr, prompt)** → SuggestionQueued?: unknown or non-claude tab →
  null. Append `SuggestedTask(id: task_xxxxxxxx)`; while > 20 → evict oldest
  (`_settled[id] = dismissed`). `_notice(suggested, suggestionId, detail = title or null if
  empty)`. save, notify. Return (task, pending copy, evicted).
- **withdraw(tabId, taskId)** → (withdrawn, already?): not found → (false, _settled[taskId]);
  else `_settle(dismissed)` → (true, null).
- **_settle(tab, task, end)**: remove from suggestions; `_settled[id] = end`; save; notify.
- **suggestionOf(notice)**; **dismissSuggestion**; **runSuggestionHere(tab, task, prompt?)**:
  exited or hibernated → banner!(`'<title> não está rodando — retome a conversa antes'`),
  false; else settle(started), `submit(prompt ?? task.prompt)`, true.
- **startSuggestionHere(tab, task, prompt?, label?)**: settle(started); `openClaude(tab.folder,
  cwd: tab.cwd, label: label ?? (title empty ? null : title), feature of tab, prompt ??
  task.prompt)`; select.
- **startSuggestionInPlugin / suggestionTracked**: xref plugins. Prompt composition for the
  tracked case: join with `'\n\n'`: trimmed intro (if non-blank), task.prompt, and (worktree
  choice) `'Você está na branch `<branch>`. Leia o código antes de propor o plano.'`; worktree
  choice uses `newTask(folder, taskId: ref, branchPattern: branch ?? 'feature/<ref>',
  dirPattern: dir ?? ref, …)`. Missing pending → banner(`'a tarefa <ref> foi criada, mas a
  sugestão já não estava mais no cartão'`).

### 3.20 Colour precedence

- **chosenTintOf(tab)** = `feature.tint ?? tab.tint ?? folder.tint ?? tint of the first
  workspace (in sidebarRows order) containing the folder`.
- **tintOf(tab)** = `chosenTintOf(tab)?.color ?? groupOf(tab)?.color`.

### 3.21 Plugins/screens (xref plugins spec)

Store holds these but they belong to the plugins subsystem: `screen`, `_parked`,
`releaseView`, `returnView`, `_syncScreen`, `_landScreen`, `openScreenHome`,
`_syncSidebar`, `updatePluginSidebar`, `railPlugins`, `moveRailPlugin`, `shownPlugin`,
`viewsOf`, `footerCommands`, `openPluginView`, `updatePluginView`, `closePluginView`,
`pluginModal`/`closeModal`/`dockModal`, `pluginViewAction`, `runPluginCommand`
(+ `expandCommand`: replace `${name}` with value, shell-quoted when `quote`, unknown names
left literal), `setPluginEnabled`, `uninstallPlugin`, `setPluginSetting`, `installPlugin`,
`ownedBy`. The core store must expose hooks for them: `_enter`, `_place`, `_placeBeside`,
`closeTab`, `save`, `notify`, and emit `session.opened/exited/closed/status`, `hook`.

---
## 4. External commands

### 4.1 The login shell (`Sh`)

Every external command (git, editor, `claude agents`, plugin background commands) runs as
**`<Sh.shell> -lc "<command string>"`** with `cwd` (optional) and environment `Sh.env`.
stdout/stderr are captured and **trimmed**. A spawn failure (ProcessException) →
`ShResult(code: -1, stdout: '', stderr: <message>)`. `ok = code == 0`.
The shell runs in its own process group; when a run is abandoned (caller drops it) or passes
its optional time limit, the whole group gets SIGKILL and a timed-out run answers
`ShResult(code: 124, stdout: '', stderr: 'timed out after <ms> ms')`.

`Sh.shell` (computed once):
- macOS → `/bin/zsh`.
- else `$SHELL` if set, non-empty and the file exists;
- else first existing of `/bin/bash`, `/usr/bin/bash`, `/bin/zsh`, `/usr/bin/zsh`;
- else `/bin/sh`.

`Sh.q(v)` = `'` + v with every `'` replaced by `'\''` + `'` (POSIX single-quote).

`Sh.env` = the process environment with
`PATH = pathWithFallbacks(inherited PATH)`:
```
dirs = inherited.split(':').filter(non-empty)
for d in binDirs: if d ∉ dirs && isDir(d): dirs.push(d)
return dirs.join(':')
```
`binDirs` (in order; `$HOME`-based ones only if HOME non-empty):
`$HOME/.local/bin`, `$HOME/.claude/local`, `$HOME/.bun/bin`, `$HOME/.volta/bin`,
`$HOME/.npm-global/bin`, `$HOME/.pub-cache/bin`, `$HOME/.cargo/bin`,
`/opt/homebrew/bin`, `/opt/homebrew/sbin`, `/usr/local/bin`, `/usr/local/sbin`,
and on Linux `/home/linuxbrew/.linuxbrew/bin`.

`Sh.profileFile` = `~/.zprofile` (macOS) / `~/.profile` (Linux) — literal `~`, used in text.

### 4.2 PTY launches (xref PTY spec, interface used by the store)

- Shell panel: `Pty.start(Sh.shell, ['-l'], cwd, env)`.
- Command panel: `Pty.start(Sh.shell, ['-lc', 'exec ' + command], cwd, env)` after writing a
  dim note line `"\x1b[2m$ <display ?? command>\x1b[0m\r\n"` to the terminal.
- env = `Sh.env` + `FORCE_HYPERLINK=1` (+ per-call extras).
- Missing cwd → no process; writes `"\x1b[31ma pasta <cwd> não existe — nada foi
  iniciado.\x1b[0m\r\n"`, exited with code −1, `onExit` fired on next microtask.
- Exit → writes `"\r\n\x1b[2m[processo saiu com <code>]\x1b[0m\r\n"`, then `onExit`.
- `remark(t)` writes `"\x1b[2m[<t>]\x1b[0m\r\n"`.
- `kill()`: SIGHUP to process group (`-pid`, fallback pid), wait up to **3 s** for exit,
  then SIGKILL. `hangUp()`: SIGHUP only. `park()`: mark exited without a process (only
  before any start). `relaunch(cmd, cwd)`: only if exited; writes `\r\n`, startCommand.

### 4.3 git (all via §4.1, `cwd` as given)

| purpose | command string | cwd | parsing / result |
|---|---|---|---|
| `mainRoot(path)` | `git worktree list --porcelain` | path | !ok → null; first line starting `worktree ` → `substring(9).trim()`; none → null |
| `worktrees(root)` | `git worktree list --porcelain` | root | !ok → `[]` if *not a repo* else **null** (keep previous); parse blocks: `worktree <path>` starts entry (flush previous), `branch <ref>` → strip `refs/heads/`, line starting `prunable` → prunable; branch default `(detached)`; first entry isMain |
| `branchOf(path)` | `git rev-parse --abbrev-ref HEAD` | path | ok → stdout else `''` |
| `dirtyCount(path)` | `git status --porcelain` | path | !ok → 0 if not a repo else null; empty → 0; else count non-blank lines |
| `defaultRemoteRef(root)` | `git symbolic-ref --quiet refs/remotes/origin/HEAD \|\| echo` then `git rev-parse --verify --quiet '<name>'` | root | if first ok and non-empty: name = stdout minus `refs/remotes/`; verify ok → name. Else try `git rev-parse --verify 'origin/master'`, then `'origin/main'`; first ok wins; else null |
| `addWorktree(root, dirName, branch, baseRef)` | 1) `test -d '<root>/.claude/worktrees/<dirName>'` (no cwd) 2) `git fetch origin --quiet` (abandoned after 60 s) 3) `git worktree add '<path>' -b '<branch>' '<baseRef>'` | –, root, root | exists → (false, path, `'Worktree já existe em <path>'`); fetch result ignored; add !ok → (false, path, stderr or stdout); ok → (true, path, `'Worktree criada em <path> (branch <branch>)'`) |
| `safetyOf(w, root)` | `dirtyCount(w.path)`; `defaultRemoteRef(root)`; `git rev-list --count '<base>'..HEAD` | w.path | prunable → (0,0); count parse failure → base=null, unmerged 0. `risky = dirty != 0 \|\| unmerged > 0`; `forceBranch = base != null && risky` |
| `removeWorktree(root, w, force, forceBranch?, deleteBranch)` | isMain → (false, `'Essa é a checkout principal — ela não se remove'`). prunable → `git worktree prune`; else `git worktree remove [--force ]'<path>'`. Then if deleteBranch && branch ≠ `(detached)`: `git branch -D\|-d '<branch>'` (`-D` iff `forceBranch ?? force`) | root | !ok → (false, stderr or stdout). Message parts joined `' · '`: `'Registros fantasmas limpos'` (prunable) or `'Worktree removida: <basename>'`; branch: `'branch <b> apagada'` or `'branch mantida (<stderr or stdout>)'` |
| `prune(root)` | `git worktree prune -v` | root | !ok → (false, stderr or stdout); count non-blank lines of `stdout + '\n' + stderr`: 0 → `'Nenhuma worktree fantasma pra limpar'`, else `'<n> registro(s) limpo(s)'` |

*Not a repo* (`_notARepo`): stderr contains `not a git repository`, `not a work tree`, or
`no such file or directory`.

### 4.4 claude

**Launch** (`_launchClaude`), executed through the PTY command path (§4.2, i.e.
`<shell> -lc "exec <cmd>"`). `cmd` = space-join of:
1. `claude`
2. `--mcp-config <q(hooks.mcpConfigFor(tabId))>` — **must stay first**: `--mcp-config`
   is variadic and would swallow the positional prompt; `--name` terminates it.
3. `--name <q(name)>` where `name = agentName ?? basename(cwd)`
4. `--settings <q(hooks.settingsFor(tabId, cwd))>`
5. `--append-system-prompt <q(brief)>` if the tab's feature has a non-blank (trimmed) brief
6. `--resume <q(resumeId)>` if resumeId
7. `-- <q(trimmed prompt)>` if non-blank (positional, last; `--` keeps a prompt starting with `-` from being read as an option)

`mcpConfigFor` and `settingsFor` return **inline JSON strings** (xref hooks):
`{"mcpServers":{"<TaskMcp.server>":{"type":"http","url":"http://127.0.0.1:<port>/mcp/<tabId>","alwaysLoad":true}}}`
and the hooks/permissions/statusLine settings JSON pointing at
`http://127.0.0.1:<port>/hook/<tabId>`.

Display line (what the terminal shows instead of the huge command):
`claude --mcp-config <tarefas :<port>> --name <q(name)> --settings <hooks :<port>>`
+ `' --append-system-prompt <briefing de <feature.name>>'` if brief
+ `' --resume …'` if resume + `' <prompt inicial>'` if prompt.

**Agents poll** (`AgentsWatcher`): `claude agents --json` via §4.1 (no cwd).
- `start()`: tick now, then periodic every **2 s** if eager else **30 s**.
- Setting `eager` changes the period (reschedule) and, if becoming eager and running,
  ticks immediately. Initial `eager = true`.
- Tick: skip if one in flight; a run still going after **20 s** is abandoned (counts as !ok); !ok → ignore; parse from the first `[` in stdout; must be a
  list; objects → `AgentInfo`; set `latest`, broadcast. Any error swallowed.

### 4.5 Editor

`Editor.open(path, line?)`:
1. `code -- '<path>'` or, with line, `code -g -- '<path>:<line>'` (one quoted arg).
2. If !ok on Linux: `code-insiders -- '<path>'`; then `xdg-open '<path>'` (result = its ok).
3. If !ok on macOS: `open -a 'Visual Studio Code' '<path>'`.
`xdg-open` and `open` have no `--`: an operand starting with `-` (path, url) is passed as `./<operand>`.

### 4.6 Other system actions (xref notify/platform)
`Notifier.alert(title, body)`, `Notifier.badge(text?)`, `Notifier.openLink(url)`,
`Notifier.quickLook(path)`, `Notifier.chooseMarkdown(startIn)` — platform channel; the core
must call them through a trait.

---
## 5. Test cases (language-neutral)

Common fixtures:
- **S1** = store with folder `F(/repo, "meu-repo")` (isRepo, branch master where noted).
- `panel(name)` = claude tab id=name, folder F, cwd `/repo`, label=name, not started.
- `turn(tab)` = applyHook `UserPromptSubmit {}` then `Stop {}`.
- `silent(tab, by=5s)` = `hooks.lastEventAt = now - by`.
- `look(tab)` = `panes = Leaf(tab)`, focus = tab.
- Tests run with a temp state dir; config assertions wait up to 3 s for the debounced write.
- `[UI]` = assertions about widgets; port to the UI spec, store-side effects listed here.

### 5.1 workspace_test

| # | given | when | then |
|---|---|---|---|
| W1 | paths | `looksLikeOne` | true for `/repos/cefis.code-workspace` and `'  /repos/Cefis.Code-Workspace  '`; false for `/repos/cefis`, `/repos/settings.json` |
| W2 | `{"folders":[{"path":"/a"},{"path":"/b"}]}` at `/repos/cefis.code-workspace` | parse | name `cefis`; paths `[/a,/b]` |
| W3 | relative `api`, `../outro/web`, `./aqui` at `/Volumes/repos/cefis/tudo.code-workspace` | parse | `/Volumes/repos/cefis/api`, `/Volumes/repos/outro/web`, `/Volumes/repos/cefis/aqui` |
| W4 | `/a/b/../b/./c`, `/../x` | parse | `/a/b/c`, `/x` |
| W5 | `~/repos/api` | parse | `$HOME/repos/api` |
| W6 | `{"path":"/a","name":"API"},{"path":"/b"}` | parse | first name `API`, second null |
| W7 | entries `{name only}`, `{path:""}`, `{path:"/b"}` | parse | `[/b]` |
| W8 | `{"settings":{}}` | parse | folders empty (not null) |
| W9 | `{"folders": [`, `[1, 2]`, `''` | parse | null each |
| W10 | JSONC with `//` comment, `/* */`, trailing commas in array and object | parse | `[/a,/b]` |
| W11 | name `http://x/y`, setting `https://exemplo.com//x` | parse | name kept `http://x/y` |
| W12 | name `b, }` | parse | kept verbatim |
| W13 | name `c:\\` (escaped backslash) then `/b` | parse | paths `[/a,/b]` |
| W14 | temp dir with `api`,`web` + `cefis.code-workspace` listing them | `importWorkspace` | 2 added; folders contain both; single workspace with those roots in order and `codeWorkspacePath` = file; banner `Workspace "cefis": 2 pastas adicionadas` |
| W15 | entry `{"path":"api","name":"API do cefis"}` | import | folder name `API do cefis` |
| W16 | `api` listed twice | import | 1 folder; added 1, already 1; banner `Workspace "cefis": 1 pasta adicionada, 1 já estava aqui` |
| W17 | `api` + missing `sumiu` | import | added 1; missing ends with `/sumiu`; sticky; banner `Workspace "cefis": 1 pasta adicionada, 1 não existe no disco` |
| W18 | same file | import twice | 2 folders; second: added empty, already 2; banner `Workspace "cefis": 2 já estavam aqui` |
| W19 | second file `outro.code-workspace` listing the same folder | import both | 1 folder, 2 workspaces, `workspacesOf(folder).length == 2` |
| W20 | `/nao/existe.code-workspace` | import | null; banner starts `Não consegui ler esse workspace` |
| W21 | file `{"folders": []}` named `vazio` | import | null; banner `O workspace "vazio" não lista nenhuma pasta`; no folders |
| W22 [UI] | add-folder dialog | type `/nao/existe.code-workspace`, Adicionar | routed to import (banner as W20), no folders. Dialog texts: `Escolher workspace…`, `Um .code-workspace adiciona todas as pastas dele de uma vez.`, picker failure `Não consegui abrir o seletor — cole aí em cima o caminho do .code-workspace` |
| W23 | import of `api`,`web` | — | workspace name `cefis`, codeWorkspacePath, 2 folders |
| W24 | file listing only missing `sumiu` | import | no workspace created |
| W25 | Workspace(id ws1, name atrium, collapsed, path, roots [a,b], collapsedFolders {b}, tint cyan) | toJson→fromJson | all equal; `Workspace(ws2, uplii).toJson()` lacks `codeWorkspacePath` and `collapsed`; `fromJson({'id':'x'})` null; `fromJson('nada')` null |
| W26 | `{path, name: atrium, collapsed: true}` | fromJson | id non-empty, codeWorkspacePath = path, collapsed, roots empty |
| W27 | folders api, solta, web; workspace cefis roots [api, web]; rootOrder empty | sidebarRows | `[ws cefis, folder solta]`; foldersOf(cefis) names `[api, web]` |
| W28 [UI] | same | render | shows `cefis`, `2 pastas`, api, web, solta; collapsing hides api/web but keeps `2 pastas` and solta; search `google` (tab labelled `permissão do google` in api) shows collapsed workspace expanded; search `nada disso` hides `cefis` |
| W29 | same | removeFolder(api), removeFolder(web) | workspace roots empty; still in sidebarRows |
| W30 | same | closeWorkspace(cefis) | workspaces empty; folders `[solta]`; banner `Workspace "cefis" fechado — 2 pastas saíram da lateral` |
| W31 | + tab1 in api, tab2 in solta | closeWorkspace | tabs `[tab2]` |
| W32 [UI] | workspace context menu | — | `Fechar workspace…` → dialog `Fechar cefis?`, `2 pastas saem da lateral`, `1 sessão aberta é encerrada`, `Nada é apagado do disco — nem os repos, nem o .code-workspace`; Cancelar closes nothing; confirm `Fechar workspace` removes section |
| W33 | 2.4.0 fixture | readSidebar | workspace `atrium`: path `/repos/atrium.code-workspace`, roots `[/repos/atrium-backend, /repos/atrium-frontend]`, collapsed true |
| W34 | 2.4.0 fixture | readSidebar | workspace `uplii` created: path `/repos/uplii.code-workspace`, roots `[/repos/uplii-ai]` |
| W35 | 2.4.0 fixture | readSidebar | sidebarRows `[ws:atrium, solta, ws:uplii]`; single feature with folderRoot `/repos/atrium-backend` |
| W36 | 2.4.0 fixture | readSidebar | `isFolderCollapsed(atrium-frontend, within: atrium)` true |
| W37 | folders atrium-api, atrium-web, infra | `createWorkspace(' ATRIUM ', [api, web])` | name `ATRIUM`; no path; foldersOf `[atrium-api, atrium-web]`; infra standsAlone; api not |
| W38 | ATRIUM [infra], UPLII [] | addToWorkspace(infra, UPLII) | workspacesOf(infra) == [ATRIUM, UPLII]; UPLII has infra |
| W39 | ATRIUM | addToWorkspace(infra) twice | roots `[/repos/infra]` |
| W40 | ATRIUM [api, web] | addToWorkspace(infra, before: web) | `[atrium-api, infra, atrium-web]` |
| W41 | ATRIUM [api] | removeFromWorkspace(api, ATRIUM, at: infra) | foldersOf empty; rows `[atrium-web, atrium-api, infra, ws:ATRIUM]` |
| W42 | ATRIUM [infra], UPLII [infra] | removeFromWorkspace(infra, ATRIUM) | infra not standing alone; root folders `[atrium-api, atrium-web]` |
| W43 | ATRIUM [api, web] + shell tab in api | dissolveWorkspace | no workspaces; 1 tab; rows `[infra, atrium-api, atrium-web]` |
| W44 | ATRIUM [api, infra], UPLII [infra] | closingWith(ATRIUM) / closeWorkspace | `[atrium-api]`; folders `[atrium-web, infra]`; workspaces `[UPLII]` with infra |
| W45 | ATRIUM & UPLII both [infra] | removeFolder(infra) | both roots empty |
| W46 | store (disposed), createWorkspace('W') | removeFolder(atrium-api); re-add Folder(/repos/atrium-api) | rows `[atrium-web, infra, ws:W, atrium-api]` |
| W47 | infra in ATRIUM & UPLII | toggleFolderCollapsed(infra, within ATRIUM) | collapsed in ATRIUM, not in UPLII; `infra.collapsed` false |
| W48 | — | createWorkspace('   ') | throws ArgumentError; none created |
| W49 | ws `atrium` | rename `'  ATRIUM '`, tint red, link path; link null | name `ATRIUM`, tint red, path set; then null |
| W50 | infra in ATRIUM(red), UPLII(cyan); shell tab in infra | chosenTintOf | red; after setFolderTint(infra, green) → green |
| W51 | no config file | createWorkspace('ATRIUM',[api]) | saved: workspaces single `{name ATRIUM, folderRoots [/repos/atrium-api]}`; rootOrder `[folder:/repos/atrium-web, folder:/repos/infra, workspace:<id>]`; no folder has key `workspace` |

### 5.2 groups_test

Fixture **G3**: folder `/repo` "meu-repo"; shell tabs `um`,`dois`,`tres` (labels same, cwd
/repo, not started); `panes = row[Leaf um, column[dois, tres](.5,.5)](.6,.4)`; focus `um`.
`fourth` = shell tab `quatro` (not on screen).

| # | given | when | then |
|---|---|---|---|
| G1 | G3 | groupPanes | name `3 terminais`, count 3, 1 group, all tabs stamped |
| G2 | G3, panes = Leaf um | groupPanes | null; no groups |
| G3 | G3 | groupPanes twice | 2 groups: `3 terminais`, `3 terminais (2)`; different ids |
| G4 | grouped | ungroup | no groups; 3 tabs; groupIds null; 3 panes on screen |
| G5 | grouped | dismiss(tres); saveGroup(same name) | tres groupId null; others stamped; group count 2 |
| G6 | grouped + fourth | groupOf colours | one colour for members ∈ groupTints; fourth has no group |
| G7 | grouped | toJson→jsonEncode→fromJson | same colour |
| G8 | grouped | swap tres→quatro in tree | activeGroup null; showing false |
| G9 | grouped + fourth | select(quatro) | panes order `[quatro]`; focus quatro; 4 tabs; group kept; activeGroup null |
| G10 | grouped | select(tres) | still 3 panes |
| G11 | G3 not grouped + fourth | select(quatro) | 3 panes, order `[quatro, dois, tres]` |
| G12 | G3; um has touched & done | saveGroup('grid de 3') | count 3; summary `3 terminais`; where `repo`; labels `[um,dois,tres]`; recipes lack `touched`, `done`, `group`; tree `{axis:row, children:[0, {axis:column, children:[1,2]}]}` |
| G13 | G3, no config | groupPanes; read config | groups has 1; round-trip equal id/name/count/tree; every layout pane has `group` = id; panes=null; openGroup(back) → order `[um,dois,tres]` |
| G14 | grouped; select(fourth) | openGroup | order `[um,dois,tres]`; still 4 tabs; root row, weights[0]≈0.6, right column `[dois,tres]`; focus um; showing true |
| G15 | saveGroup('grid') | dismiss(tres); saveGroup('GRID') | 1 group, same id, count 2, name `GRID` |
| G16 | panes null | saveGroup('vazio'), saveGroup(''), groupPanes | all null; no groups |
| G17 | saved | removeGroup | no groups; 3 tabs; 3 panes |
| G18 | — | fromJson `{id gp1,name grid,panes []}`, `{name '  ', panes [{}]}`, `{name grid, panes [{}]}` (no id), `'grid'` | all null |
| G19 [UI] | panel context menu | `Agrupar painéis` / `Desagrupar "3 terminais"`; not offered for single pane; offered again when screen no longer the group |
| G20 [UI] | grouped, all dismissed | click row `um` | opens group (order um,dois,tres); click non-member row replaces grid |
| G21 [UI] | `Grupos` header | — | lists group with summary `3 terminais`; clicking opens grid; collapsing sets `groupsCollapsed` and shows count |
| G22 | saved group, no config | toggleGroupsCollapsed | config `groupsCollapsed: true` |
| G23 | groupPanes + saveGroup('grid da tarde') | clearGroups | returns 2; no groups; 3 tabs; panes order unchanged; groupIds null; second call returns 0 |
| G24 [UI] | clear button | tooltip `Esquecer todos os grupos` visible only on hover; 2 groups → dialog `Esquecer os 2 grupos?`; confirm `Esquecer` → banner contains `2 grupos esquecidos`; 1 group → no dialog, banner contains group name |

### 5.3 layout_test

| # | given | when | then |
|---|---|---|---|
| L1 | claude tab cwd `/repo/.claude/worktrees/TASK-47730`, branch `feature/TASK#47730`, label `refatorar auth` | title | `refatorar auth` |
| L2 | same without label | title | `TASK#47730` |
| L3 | shell, cwd `/repo/api`, branch '' | title | `api` |
| L4 | cwd `/repo` (== root), branch master | title | `meu-repo` |
| L5 | L2 tab with label `TASK#47730`, sessionId `abc-123` | toJson | exactly `{folderRoot:/repo, kind:claude, cwd:…TASK-47730, label:TASK#47730, sessionId:abc-123}` |
| L6 | shell with sessionId set | toJson | no `sessionId` |
| L7 | store | setSidebarWidth(2000) / (−40) | 680 / 248 |
| L8 | store | setSidebarWidth(420) | 420 |

### 5.4 paths_test

| # | input | expandHome |
|---|---|---|
| P1 | `~/repos/foo`, `~/` | `$HOME/repos/foo`, `$HOME/` |
| P2 | `~` | `$HOME` |
| P3 | `~outro/repos`, `~outro` | unchanged |
| P4 | `/Volumes/Externo/repos/foo`, `repos/foo`, `./foo`, `''`, `/tmp/notas.md~` | unchanged |
| P5 | temp dir created under HOME | `addFolder('~/<name>')` | non-null; root has no `~`, starts with HOME, exists |

### 5.5 hibernate_test

Fixture: S1 + claude tab `t1` label `implementa`, sessionId `sess-1`. `now = 2026-09-18 12:00`.

| # | given | when | then |
|---|---|---|---|
| H1 | fixture | canHibernate; hibernate | true; hibernated, exited, resumable, resumeId `sess-1`, subtitle contains `Hibernada`, status ended, still in tabs, canHibernate now false |
| H2 | sessionId null | hibernate | canHibernate false; not hibernated, not exited |
| H3 | shell tab | canHibernate | false |
| H4 | armed | hibernate | armed false |
| H5 | hibernated | select(tab); yield | not hibernated; subtitle lacks `Hibernada`; resumeId `sess-1` |
| H6 | not hibernated | wake | status unchanged; not exited |
| H7 | turn; restedAt = now−31min; not on screen | hibernateIdle(now) | hibernated |
| H8 | restedAt = now−29min | hibernateIdle | not hibernated |
| H9 | status ready, restedAt null | hibernateIdle(startedAt + 31min) | hibernated |
| H10 | restedAt now−2h; on screen & focused | hibernateIdle | not |
| H11 | status waitingPermission | hibernateIdle | not |
| H12 | status working, lastEventAt now−2h | hibernateIdle | not |
| H13 | followUps non-empty | hibernateIdle | not |
| H14 | another tab has handoff step targeting t1 | hibernateIdle | not |
| H15 | restedAt now−1day; setHibernateMinutes(0) | hibernateIdle | autoHibernate false; not |
| H16 | setHibernateMinutes(5); restedAt now−6min | hibernateIdle | hibernated |
| H17 | hibernated | toJson / recipe | `hibernated: true`, `sessionId: sess-1`; recipe lacks `hibernated` |
| H18 [UI] | hibernated row | — | moon icon (bedtime) and text containing `Hibernada` |

### 5.6 pin_test

Fixture: claude tabs (ids as named) in folder /repo (not in `folders`).

| # | given | when | then |
|---|---|---|---|
| N1 | row[fixo, solto], focus fixo, pin fixo | select(tres) | order `[fixo, tres]`; focus tres; fixo pinned |
| N2 | same, focus solto | select(tres) | `[fixo, tres]` |
| N3 | Leaf fixo pinned, focus fixo | select(novo) | `[fixo, novo]`; focus novo |
| N4 | row[fixo, solto], fixo pinned | dismiss(fixo) | `fixo.pinned` false; isPinned false |
| N5 | row[fixo, solto], fixo pinned | dropTab(tres → fixo, center) | `[tres, solto]`; tres pinned; fixo.pinned false |
| N6 | row[solto, column[a,b]](.4,.6), a & b pinned, focus a | dismiss(solto); select(outro) | root row, weights `[0.4, 0.6]`, first leaf outro, last `[a,b]`; both still pinned |
| N7 | row[solto, fixo], fixo pinned | closeTab(solto); select(outro) | `[outro, fixo]` |
| N8 | row[solto, a], a pinned | dismiss(solto); dropTab(b → a, bottom); pin b; focus a; select(outro) | root row: first `[a,b]`, last leaf `outro` |
| N9 | column[a,b], both pinned, focus a | select(novo) | root row: first `[a,b]` (column), last `novo` |
| N10 | row[solto, fixo], fixo pinned & focused | showDoc(MxDoc.file('/repo/README.md'), from: fixo) | order `[solto, doc, fixo]`; root last weight 0.5 |
| N11 | Leaf fixo pinned | toJson / recipe | `pinned: true`; recipe lacks `pinned` |

### 5.7 reorder_test

| # | given | when | then |
|---|---|---|---|
| R1 | tabs um,dois,tres,quatro | moveTab(um, tres) | `[dois, tres, um, quatro]` |
| R2 | same | moveTab(quatro, dois) | `[um, quatro, dois, tres]` |
| R3 | um,dois,tres | moveTab(dois, dois) | unchanged |
| R4 | tab not in list | moveTab(ghost, um) | unchanged |
| R5 [UI] | drag rows | down/up lands on target slot; across folders nothing moves; 3-px drift = click (selects, focus = row) |
| R6 | folders um,dois,tres,quatro | drop(root quatro → root dois) | `[um, quatro, dois, tres]` |
| R7 | same | drop(um → tres) | `[dois, tres, um, quatro]` |
| R8 | um,dois | canDrop(dois→dois) / drop | false / unchanged |
| R9 | um, api, web, dois; ws cefis [api,web] | drop(ws → um) | `[ws:cefis, um, dois]`; ws folders `[api, web]` |
| R10 | um,dois + api; ws [api] | drop(um → ws) | `[dois, ws:cefis, um]` |
| R11 | a [api], b [web] | canDrop(root a → web inside b) ; canDrop(a → b, into) | false; false |
| R12 | ws [api,web,app] | drop(app in ws → api in ws) | `[app, api, web]` |
| R13 | api,web,solta; ws [api,web] | drop(api in ws → root solta) | ws `[web]`; rows `[api, solta, ws:cefis]` |
| R14 | same | drop(root solta → web in ws) | ws `[api, solta, web]`; rows `[ws:cefis]` |
| R15 | a [api], b [web] | drop(api in a → b, into) | a empty; b `[web, api]` |
| R16 | api, solta; ws [api] | drop(root solta → ws, into) | ws `[api, solta]`; rows `[ws:cefis]` |
| R17 | ws [api] | canDrop(api in ws → ws, into) | false |
| R18 | solta, api; ws [api] | drop(root solta → root ws) | ws `[api]`; rows `[ws:cefis, solta]` |
| R19 [UI] | folder header drag `gama → alfa` | rows `[gama, alfa, beta]`; 3-px drift on header = click → folder collapsed |

### 5.8 worktrees_test

| # | given | when | then |
|---|---|---|---|
| T1 [UI] | folder /repo with worktrees main, TASK-47730, TASK-45371 (prunable) | sidebar | no `Worktrees` text, no task labels; ghost chip (link_off) with `1`; RepoGlyph isRepo; `master` shown |
| T2 [UI] | right-click repo | — | `Worktrees` with `3`, `Remover pasta`; only main → no `Worktrees`; submenu shows `TASK#47730`, `TASK#45371`, `Sem pasta no disco`, `Limpar worktrees fantasmas`; item menu: `Sessão do Claude`, `Terminal`, `Abrir no vscode`, `Excluir worktree…`, not `retomar conversa…` |
| T3 | `{root:/repo, name:meu-repo, collapsed:true, worktreesCollapsed:false}` | Folder.fromJson | name, collapsed true; without `collapsed` → false |

### 5.9 config_test

| # | given | when | then |
|---|---|---|---|
| C1 | test env | stateHome / configPath | under system temp; not `$HOME/.maestria` |
| C2 | claude tab with sessionId `sess-do-fundo` and a shell tab, both `term.exited = true` | renameTab → save | layout panes' sessionIds = `[sess-do-fundo]` (the shell is dropped) |
| C3 | folder /repo | toggleGroupsCollapsed → save | config folders single root `/repo`, `groupsCollapsed` true; no other file in state dir starting with `config.json` |
| C4 | `{folders:[/repo], projects:[{id pj1,…}]}` | readSidebar | feature `pj1`, name `permissão do google` |
| C5 | both `projects` (velho) and `featuresOrHotfixes` (novo) | readSidebar | ids `[novo]` |
| C6 | `{projects:[{root:/repo, name}]}` (no folders) | readSidebar | folder `/repo`; no features |
| C7 | — | featureOrHotfixIdIn | `{featureOrHotfixId:a}`→a; `{projectId:b}`→b; both → a; `{}` → null |
| C8 | folder | addFeatureOrHotfix → save | no `projects` key; `featuresOrHotfixes[0].name` = `permissão do google` |

### 5.10 features_or_hotfixes_test

| # | given | when | then |
|---|---|---|---|
| F1 | S1 | add(name, brief `contexto`) → toJson→fromJson | same id/name/brief/folderRoot |
| F2 | + folder /outro | assign(mine), assign(theirs from /outro) | mine set; theirs null |
| F3 | feature with tab | removeFeatureOrHotfix | tab stays, featureId null, no features |
| F4 | feature with 2 tabs + 1 loose tab | completeFeatureOrHotfix | returns 2; both closed; loose stays; no features |
| F5 | empty feature | complete | 0; removed |
| F6 | feature | removeFolder | no features |
| F7 | tab inside, tab outside | moveTab(outside, inside) | outside gets feature id; moveTab(outside, plain) → null |
| F8 | feature on `store.loose` | featuresOrHotfixesOf | folderRoot = loose.root; listed under loose, not under F; loose tab assignable |
| F9 | loose feature | assign(tab of F) | refused (null) |
| F10 | loose feature | round trip | folderRoot loose.root, name kept |
| F11 [UI] | loose tray feature | drawn in tray; its tab indented further than loose tab; menu `O que fazer com essa feature` has `Sessão do Claude`, not `Nova task nessa feature…` |
| F12 [UI] | feature panels | drawn only inside feature; collapsed feature hides panels |
| F13 | add(kind hotfix) | round trip | kind hotfix; toJson kind `hotfix`; fromJson without kind → feature |
| F14 | feature | setKind(hotfix) then (feature) | follows |
| F15 [UI] | hotfix + feature | `hotfix` label, bolt icon, track_changes icon; folder menu `Nova feature…`, `Novo hotfix…` (no `novo projeto…`); hotfix menu `Concluir hotfix`, `Dissolver hotfix`, `Virar feature` |

### 5.11 followups_test

Fixture: S1 + claude tab t1 `implementa`. `step(x)` = keepGoing x.

| # | given | when | then |
|---|---|---|---|
| Q1 | queue [um, dois] | turn; silent; pump | `[dois]`; again → empty |
| Q2 | queue [um, dois] | UPS, Stop, Stop; silent; pump ×2 | `[dois]` |
| Q3 | queue [revisa] | UPS, PreToolUse Agent ×2, Stop; silent | status idle; holdFor = forks; pump → 1 left. PostToolUse{agent_id a1}, SubagentStop{a1}; silent; pump → still 1. SubagentStop{a2}; silent; pump → empty |
| Q4 | queue [revisa] | UPS, PreToolUse Agent, Stop, Notification idle_prompt | atRest; SubagentStop a1; silent; pump → empty |
| Q5 | queue [segue] | UPS, PreToolUse Task, Stop; silent(3min) | holdFor go; pump → empty |
| Q6 | queue [um]; turn | holdFor | quiet; pump → 1 left; silent; pump → empty |
| Q7 | t2 panel; queue [handoff→t2 'assume', step] | turn; silent; pump | remaining kinds `[keepGoing]`; armed true; silent; pump → empty |
| Q8 | extra tab t9 after t1; queue [command `flutter analyze && flutter test`] | turn; silent; pump; yield | new shell `bornOf == t1`; descendsFrom true; index = t1+1; last tab is t9; title `flutter analyze && flutter…` |
| Q9 | no queue | turn; silent; pump | empty; armed false |
| Q10 | queue [velho]; queue [novo, outro] | — | `[novo, outro]` |
| Q11 | turn; silent | queue [vai] ; pump | 1 left (arming isn't firing); queue [vai] now:true; pump → empty |
| Q12 | queue [um]; turn | armed true; closeTab; silent; pump | armed false; followUps still 1 |
| Q13 | queue [um,dois]; turn | setDone(true) | returns 2; after pump: empty, armed false |
| Q14 | lastMessageFull `Troquei o endpoint pra /v2/permissions.\nFalta o refresh token.` | handoffText(note `Pega daqui e termina o refresh.`) | contains `"implementa"`, `Falta o refresh token.`, the note |
| Q15 | no message | handoffText('assume') | contains `assume`; with message `pronto` and empty note → contains `pronto`, trimmed text ends with `"""` |

### 5.12 rested_test

| # | given | when | then |
|---|---|---|---|
| E1 | at = 2026-09-09 12:00 | shortAgo(at+Δ) | 3s `agora`; 59s `agora`; 1min `1min`; 59min `59min`; 3h `3h`; 2d `2d`; 9d `9/9` |
| E2 | fixture (S1 + t1) | turn | restedAt set; rested; restedAgo `agora`; unseen true |
| E3 | look(t1) | turn | rested; unseen false |
| E4 | look; setWindowActive(false) | turn | unseen true |
| E5 | turn | UPS | restedAt null, not rested, unseen false |
| E6 | turn | Stop again | restedAt unchanged |
| E7 | turn (unseen) | seeFocused (not on screen) | false, still unseen; look + inactive → false; active → true & unseen false; again → false |
| E8 | inactive; turn | setWindowActive(true) | 1 toast; banner null; unseen true |
| E9 | inactive; turn; dismissToast | active | banner `implementa terminou enquanto você estava fora`; unseen |
| E10 | turn; clearBanner | inactive; active | unseen; banner null |
| E11 [UI] | rested row | shows `agora` in green while unseen; non-green after seen |

### 5.13 session_done_test

| # | given | when | then |
|---|---|---|---|
| D1 | panel | setDone(true) | done; still in tabs |
| D2 | panel | toJson before/after | `done` absent / true |
| D3 | queue [um,dois] | setDone(true) | 2; queue empty |
| D4 | done | setDone(true) | 0; still done |
| D5 | feature panel waitingPermission | needingHuman(F), needingHumanIn(feat) | 1,1; after done → 0,0 |
| D6 | done | applyHook UPS | done false |
| D7 | — | UPS, Stop | status idle; done false |
| D8 | done + pending | closeSettled(F) | tabsOf(F) == [pending] |
| D9 | waitingPermission, done | setDone(false) | returns 0; not done; needingHuman 1 |
| D10 [UI] | row | tick icon (task_alt) when done; badges disappear |

### 5.14 waiting_states_test [UI]

asking: waitingAnswer, question `Receitas`, questions 1; gated: waitingPermission, activeTool `Bash`.
Subtitles `Pergunta: Receitas` and `Quer aprovação pra Bash`; no `AskUserQuestion` text;
exactly one lock icon and one question-mark icon; count pills have different colours.

### 5.15 search_test

| # | given | when | then |
|---|---|---|---|
| Q-S1 | panels `permissão do drive`, `migração do banco` | setQuery('drive') | matches first only |
| Q-S2 | panel `um` in feature `permissão do google`, branch `feature/TASK#47730` | query `meu-repo`, `google`, `TASK#47730` → match; `shell` → no (it's claude) |
| Q-S3 | `permissão do google` | `google perm` → match; `google banco` → no |
| Q-S4 | tabs in /repo and /outro | toggleFilterFolder(/repo) | only /repo |
| Q-S5 | here (/repo), there (/outro, in feature), neither (/outro) | filter folder /repo + feature | here ✓, there ✓, neither ✗ |
| Q-S6 | waiting(perm), working, shell | filters claude+waiting | waiting ✓ only; + working → working ✓ too |
| Q-S7 | done & waitingPermission | filter waiting → ✗; swap to finished → ✓ |
| Q-S8 | query + filter shell | hits empty; clearSearch → filtering false, query '', activeFilters 0, hits 1 |
| Q-S9 [UI] | sidebar search | hides non-matching rows and empty folders; reveals collapsed folder content without changing `collapsed`; `Nenhuma sessão…` + `Mostrar tudo` clears; narrow (248 px) layout; `SidebarSearch.reveal` focuses field |

### 5.16 loose_tray_test [UI + store]

`Avulsos` divider (no chevron, no glyph); loose panels less indented; count shows only when
non-empty; `Abrir algo sem pasta` menu offers `Sessão do Claude`, `Terminal`, `Feature…`;
folder `Abrir algo nessa pasta` also `Feature…`; + buttons visible only on hover.
Store-relevant: with one done and one running loose panel, sweep → `tabsOf(loose)` titles
`[ainda roda]`, banner contains `Um painel fechado`; exited panel is **not** swept
(`[ainda roda, saiu na largada]`); nothing done → banner contains `Nada pra limpar aqui`;
sweep button tooltip `Limpar 1 painel concluído` hidden while a query is active.

### 5.17 shell_env_test

| # | given | when | then |
|---|---|---|---|
| X1 | bare `/usr/bin:/bin:/usr/sbin:/sbin`, candidate `/opt/claude/bin` exists | pathWithFallbacks | bare + `:/opt/claude/bin` |
| X2 | + `/opt/nao-instalado/bin` missing | — | only existing appended |
| X3 | candidate already in PATH (listed twice) | — | not duplicated |
| X4 | empty PATH | — | `/opt/claude/bin` |
| X5 | machine | Sh.env PATH | contains every existing binDir |
| X6 | — | earlyExitMessage('maestria_v2', 127) | starts `maestria_v2: `, contains `PATH` and profileFile |
| X7 | — | code 1 / null | contains `código 1` and `abra o painel` / `código ?` |

### 5.18 history_test

Fixture tree `test/fixtures/history` (copy verbatim into the Rust repo):
- `-tmp/aaaa1111-…-000000000001.jsonl` (titled: custom-title/agent-name `maestria_v2`, user
  `arruma o cabecalho…` cwd `/tmp`, later `ai-title` `Cabeçalho da lateral com a busca`),
  plus `-tmp/aaaa1111-…/subagents/agent-aaf825f9327fff8cc.jsonl`.
- `-tmp/bbbb2222-…2.jsonl` (spoke: custom/agent name, sidechain user line, user line with
  image + text `<ide_opened_file>…</ide_opened_file>\n\n## Tarefa\no + da bandeja de grupos
  ficou torto`, cwd `/tmp`, then a truncated assistant line).
- `-tmp/cccc3333-…3.jsonl` (mute: only names + queue-operation).
- `-repo-sumiu/dddd4444-…4.jsonl` (orphan: ai-title `Migrar o schema do relatório`, cwd
  `/repo/sumiu`).

| # | given | when | then |
|---|---|---|---|
| Y1 | fixtures | read(cwds [/tmp]) | ids {titled, spoke, mute}; no `agent-…`; no orphan |
| Y2 | — | titled.title | `Cabeçalho da lateral com a busca` |
| Y3 | — | spoke.title | `Tarefa o + da bandeja de grupos ficou torto` |
| Y4 | — | mute.title | `maestria_v2` |
| Y5 | — | titled entry | sessionId; cwd `/tmp`; where `tmp`; bytes = file size; at = mtime; missing false |
| Y6 | — | read() (all) | 4 ids; orphan cwd `/repo/sumiu`, title `Migrar o schema do relatório`, missing |
| Y7 | NoPty store, folder /tmp | allChats / chatsIn(folder) | 4 / 3 |
| Y8 | folder /tmp; worktrees main /tmp, `/var/wt/TASK-1` | folderAt | `/tmp`, `/tmp/lib/ui`, `/var/wt/TASK-1` → folder; `/var/wt/TASK-1000`, `/tmpfs/outro`, `''` → null |
| Y9 | temp root, `velha.jsonl` then (20 ms later) `nova.jsonl` | read | titles `[nova, velha]`; limit 1 → `[nova]` |
| Y10 | — | read(cwds [/nao/existe]); read(root missing) | empty |
| Y11 | — | read(cwds [/tmp, /tmp, /-tmp]) | 3 entries |
| Y12 | NoPty, folder /tmp | standingOf(titled) fresh; resumeChat(folder) | asked `{folder /tmp, cwd /tmp, label 'Cabeçalho da lateral com a busca', resumeId titled}`; tab.resumeId titled; title = label |
| Y13 | folder /tmp | resumeChat(chat from allChats) without folder | asked folder `/tmp` |
| Y14 | no folders | resumeChat | folder = loose.root; cwd `/tmp` |
| Y15 | resumed once; panes=null | standingOf → onScreen; resumeChat | returns same tab; asked length 1; panes non-null |
| Y16 | agents.latest has pid 4242 with titled sessionId | standingOf → live; resumeChat | null; nothing asked; banner contains `ainda está rodando fora do MaestrIA` |
| Y17 | orphan | standingOf gone; resumeChat | null; banner contains `/repo/sumiu` |
| Y18 [UI] | history dialog | `Todas as conversas`, notes `a pasta não existe mais` / `Não sei onde ela rodou`, sections per folder with `Avulsos` last and counts, `Todas as conversas em tmp` → `Buscando…` → loads folder history; tooltip `Retomar uma conversa`; clicking a row resumes and closes dialog |

`NoPty` = store whose `openClaude` records `{folder, cwd, label, resumeId}`, creates the tab
(id `tab<n>`, sessionId = resumeId), adds it and `select`s it — in Rust use a fake PTY
backend instead of overriding.

### 5.19 copy_test (xref PTY/VT spec)

`selectedText` over a VT buffer: TUI-skipped columns become spaces
(`Tip:     write     code`); trailing empty cells not included; written spaces kept;
wide chars don't add a trailing space; half-selected wide char excluded; two lines →
`\n`; soft-wrapped line copied as one. ⌘C copies selection to clipboard.

### 5.20 Additional tests the Rust port should add (behaviors with no Dart test)

- Atomic write: temp name contains pid; config written within ~400 ms of last change and
  only once for a burst.
- `_restoreLayout`: legacy `active`/`split`; `active` absent & no tree → panes null;
  missing cwd → slot null and later leaves shift correctly; `focused: -1` → first pane.
- `browser` pane dropped; unknown kind restored as shell; `projectRoot` key honored.
- Load failure → `config.json.bak` created.
- `hibernated` + sessionId restores parked, `ended`, with remark; without sessionId runs.
- `sidebarRows` with dangling rootOrder keys and folders absent from rootOrder.
- Pane tree: insert same-axis split halves the target weight; remove absorbs same-axis
  child; edge scaling; resize clamping; toJson/fromJson normalization.

---
## 6. Suggested Rust layout

One library crate `maestria-core` (no UI types), each file ≤ 400 LOC. Side effects behind
traits so the store is testable without processes, clocks or a desktop.

```
crates/maestria-core/src/
  lib.rs                    re-exports
  paths.rs                  expand_home, state_home/state_dir (override for tests)
  text.rs                   utf16 clip helpers, base36, iso8601 local fmt/parse, short_ago/until
  model/
    folder.rs               Folder (+ loose ctor), serde
    workspace.rs            Workspace, new_workspace_id (global AtomicU64 seq)
    feature.rs              FeatureOrHotfix, FeatureOrHotfixKind (+ PT strings)
    group.rs                PaneGroup (summary, where, color index)
    launcher.rs             Launcher, LauncherIcon
    tint.rs                 MxTint (names only; colour mapping lives in theme crate)
    status.rs               ClaudeStatus (+labels, at_rest, needs_human, call_to_action)
    hook_state.rs           HookState, PlanNote, SuggestedTask, caps, subtitle
    followup.rs             FollowUp, FollowUpKind, FlowHold (+PT strings)
    notice.rs               MxNotice, MxNoticeKind
    tab.rs                  Tab fields, title, subtitle, recipe/to_json
    worktree.rs             WorktreeInfo (+short_label)
    agent.rs                AgentInfo, ContextUsage
    filter.rs               MxFilter, MxFilterGroup
  layout/
    tree.rs                 PaneNode, DropSide, order/has/swap/insert/remove/edge/copy/same_shape/resize
    json.rs                 to_json/from_json + normalize
  persist/
    schema.rs               serde structs for config.json (raw serde_json::Value where lenient)
    read.rs                 load + read_sidebar + legacy migrations + restore descriptors
    write.rs                build config Value, atomic tmp+rename, .bak on failure
    debounce.rs             400 ms save debouncer (tokio)
  sys/
    shell.rs                Sh: resolve shell, quote, env PATH fallbacks, run() -> ShResult
    git.rs                  Git API (§4.3)  — over a CommandRunner trait
    editor.rs               Editor::open
    agents.rs               AgentsWatcher (eager/idle cadence, in-flight guard)
    history.rs              ChatHistory::read, ChatEntry, ChatStanding
    code_workspace.rs       jsonc strip, parse, resolve/normalize, WorkspaceImport summary
  ports.rs                  traits: Term (start_shell/start_command/park/remark/kill/
                            hang_up/relaunch/submit/pid/exited/exit_code), Notifier,
                            PluginBus (emit), HookServer (port, mcp_config_for,
                            settings_for), Clock (now), CommandRunner
  store/
    mod.rs                  Store struct (§1.15), new(), change notification channel
    lifecycle.rs            init, ticks (1s/10s/20s), shutdown, dispose
    save.rs                 save()/touch()/write_config glue, pinned cleanup
    restore.rs              restore_layout, open_pane
    sidebar.rs              folders, add/import/remove, focused_folder
    workspaces.rs           workspace ops, sidebar_rows, root order, can_drop/drop
    features.rs             feature ops, assign
    tabs_open.rs            open_shell/open_claude/launch_claude/register/readers/setup
    placement.rs            select, place, place_beside, hole, drop_from_tree
    tabs_ops.rs             dismiss, close_tab, close_settled, move_tab, pin, drop_tab,
                            focus/cycle, set_done, rename/tint, reveal
    groups.rs               save/group/ungroup/stamp/open_group/adopt/clear
    launchers.rs            launcher ops, relaunch
    hibernate.rs            can_hibernate/hibernate/wake/hibernate_idle
    git_ops.rs              refresh_git, new_task, remove/prune worktree, open_in_editor
    history_ops.rs          chats_in, all_chats, folder_at, standing_of, resume_chat
    search.rs               query/filters/matches
    notices.rs              banner, notices, toasts, window activity, see_focused
    ingest.rs               apply_hook, apply_status, apply_agents, check_alerts
    flows.rs                queue, start_flow, hold_for, pump, advance, run_follow_up,
                            handoff_text, shell_label, descend
    suggestions.rs          suggest/withdraw/settle/run/start
    tint.rs                 chosen_tint_of, tint_of
```
Tests: `crates/maestria-core/tests/*.rs` mirroring §5, with fixtures copied to
`tests/fixtures/` (`config-2.4.0.json`, `history/**` verbatim).

Architecture notes:
- The Dart store is single-threaded; emulate with a single-owner `Store` driven by an
  actor task (`tokio::sync::mpsc` of commands + `tokio::sync::broadcast`/`watch` for
  "changed"), or `Rc<RefCell<Store>>` on a UI thread. Avoid fine-grained locks: every op
  reads and writes many collections.
- Tabs are referenced by id (`String`) and stored in `Vec<Tab>` (order matters); `folder`
  is a reference in Dart — in Rust store `folder_root: String` + `loose: bool` and resolve
  through the store (renames are then automatic). Same for `launcher_id`.
- All timers (debounce 400 ms, settle 3 s, banner 6 s, toast 8 s, reveal 2 s, ticks
  1/10/20 s, agents 2/30 s) go through the `Clock`/scheduler port so tests can advance time.
- Async git/agent calls must operate on snapshots and re-check membership after `await`
  (the Dart iterates copies for this reason).

Crates:
- `serde`, `serde_json` (feature `preserve_order`) — config I/O, lenient `Value` access.
- `tokio` (`rt`, `time`, `process`, `fs`, `sync`, `macros`) — timers, subprocesses.
- `chrono` — local ISO-8601 timestamps, calendar-day comparison for history `on`.
- `regex` — title/shortLabel/plan headline patterns.
- `rand` (or `getrandom`) — `task_` ids from OS RNG.
- `thiserror` (errors), `tracing` (logs).
- `nix` or `libc` — process-group signals (PTY subsystem, xref).
- `portable-pty` — PTY subsystem (xref).
- dev: `tempfile`, `filetime` (mtime ordering in history tests), `pretty_assertions`.

---

## 7. Quirks and decisions to keep (or consciously change)

1. **rootOrder written = derived `sidebarRows`**, not the raw field: unknown keys vanish
   on the next save, and new rows become explicit.
2. **Partial load then overwrite** (§2.4): only `config.json.bak` protects the user. Keep
   the `.bak`; prefer per-entry leniency.
3. **Workspace/PaneGroup/Launcher type errors abort the load** in Dart; Folder/Feature
   errors skip the entry. Recommended: skip in all cases.
4. `applyAgents` updates `sessionId`/`agentName` without saving; persistence happens on the
   next unrelated save (hook events with `session_id` *do* save). Saving there is harmless.
5. `removeGroup` does not clear `groupId` stamps on tabs (they persist as `group` in the
   layout until the next restore drops unknown ids). `ungroup`/`clearGroups` do clear.
6. `closeTab` leaves `followUps` on the removed tab object (tests rely only on `armed=false`).
7. `renameFolder`/`renameTab`/`addFeatureOrHotfix` do not trim or validate the name.
8. `subtitle` for waitingPermission with null tool prints `null` (Dart interpolation);
   Rust should mirror (`Quer aprovação pra null`) or decide on `''` — cosmetic.
9. `Workspace.fromJson` with an empty id and a legacy `path` generates a **new id on every
   load** until saved (the save writes the new id, so it stabilizes after one save).
10. `newWorkspaceId` combines µs time with a process-global counter so that a migration
    creating several workspaces in the same µs gets distinct ids — keep the counter.
11. Booleans are omitted when false; `Folder.collapsed`, `FeatureOrHotfix.collapsed`,
    `brief`, `kind` are always written; `Workspace.collapsed` only when true.
12. The `dictation` key is intentionally dropped on save.
13. Under test, the Dart code switches state home and chat home on `FLUTTER_TEST`; the Rust
    port must use explicit injection instead (never read the real `~/.maestria` or
    `~/.claude` in tests).
14. Two app instances share `config.json`; last writer wins. The pid in the temp name only
    prevents torn writes, not lost updates.
