# mx-sys — API pública

Efeitos de SO com tokio. Todo comando externo passa por `CommandRunner`
(`mx_core::ports`, re-exportado aqui): produção usa `LoginShell`, testes usam `FakeRunner`.
Funções `*_on(…, Platform, …)` existem para testar Linux/macOS em qualquer máquina.

## Raiz
- `Platform { Linux, MacOs }`, `Platform::current()`.
- `CommandRunner`, `CommandOutput` (= `ShResult`), `BoxFuture` — re-export de `mx_core::ports`.
- `output(code, stdout, stderr) -> ShResult`.
- `LoginShell` — runner real: `<shell> -lc "<cmd>"`, env com PATH de fallback.
- `FakeRunner` — `respond(prefix, r)` (fixo), `respond_once(prefix, r)` (consumido, tem
  prioridade), `with_fallback(r)` (padrão: code 1), `calls() -> Vec<runner::Call{command,cwd}>`,
  `commands()`.

## shell (spec 01 §4.1)
- `shell() -> &str` — shell resolvido uma vez; `resolve_shell(platform, $SHELL, exists)`.
- `q(v)` — aspas simples POSIX (`'` → `'\''`).
- `env() -> &HashMap` — ambiente do processo com `PATH = path_with_fallbacks(...)`.
- `path_with_fallbacks(inherited, candidates, exists)`, `bin_dirs(home, platform)`.
- `profile_file(platform)` — `~/.zprofile` / `~/.profile`.
- `early_exit_message(title, code, platform)` — banner do claude que saiu em < 5 s.
- `run(cmd, cwd) -> ShResult` — stdout/stderr trimados; falha de spawn → code −1; sinal → −signo.
- `stderr_or_stdout(&r)`.

## git (spec 01 §4.3) — `Git::new(Arc<dyn CommandRunner>)`
- `main_root(path) -> Option<String>`
- `worktrees(root) -> Option<Vec<WorktreeInfo>>` — `Some([])` se não é repo, `None` = manter o anterior.
- `branch_of(path) -> String`, `dirty_count(path) -> Option<usize>`
- `default_remote_ref(root) -> Option<String>`
- `add_worktree(root, dir_name, branch, base_ref) -> GitOutcome { ok, path, message }`
- `safety_of(&w, root) -> WorktreeSafety { dirty, unmerged, base }` + `risky()`, `force_branch()`
- `remove_worktree(root, &w, force, force_branch: Option<bool>, delete_branch) -> GitOutcome`
- `prune(root) -> GitOutcome`
- livres: `parse_worktrees(porcelain)`, `not_a_repo(&r)`, `count_lines`; re-export
  `WorktreeInfo`, `DETACHED` de `mx_core::model::worktree`.

## editor (spec 01 §4.5)
- `open(runner, path, line: Option<u32>) -> bool` — `code`, depois `code-insiders`/`xdg-open`
  (Linux) ou `open -a 'Visual Studio Code'` (macOS). `open_on(..platform..)`.

## agents (spec 01 §4.4)
- `AgentsWatcher::new(runner)`; `start()` (tick já + a cada 2 s eager / 30 s idle),
  `stop()` (também no Drop), `set_eager(bool)` (virar eager dispara tick imediato),
  `eager()`, `is_running()`, `latest() -> AgentList`, `subscribe() -> watch::Receiver<AgentList>`
  (notifica a cada poll bem-sucedido). Tick em voo → o próximo é pulado. Falhas engolidas.
- `AgentList = Arc<Vec<AgentInfo>>`, `parse_agents(stdout)`, `COMMAND`, `EAGER_EVERY`, `IDLE_EVERY`.
  Precisa de runtime tokio (testes: `start_paused`).

## history (spec 01 §2.9) — raiz sempre por parâmetro
- `home_root(home) -> PathBuf` (`$HOME/.claude/projects`), `dir_for(cwd)` (por unidade UTF-16).
- `read(root, &ReadOptions { cwds: Option<Vec<String>>, on: Option<NaiveDate>, limit }) -> Vec<ChatEntry>`
  (`ReadOptions::default()` = todas as pastas, limite 40).
- `ChatEntry { session_id, cwd, title, at: SystemTime, bytes, missing }` + `where_()`, `label()`,
  `ago(now)`, `size()`.
- `ChatStanding { Fresh, OnScreen, Live, Gone }` + `note()`, `opens()` (o cálculo é do store).
- `clean(text)` — regra de título.

## code_workspace (spec 01 §2.10)
- `looks_like_one(p)`, `read(raw)` / `read_with(raw, home)` (async), `parse(src, path)` /
  `parse_with(src, path, home)` → `Option<CodeWorkspace { path, name, folders: Vec<WorkspaceFolder { path, name }> }>`.
- `strip_jsonc`, `resolve`/`resolve_with`, `normalize`, `name_of`, `EXT`.
- `WorkspaceImport<F> { workspace, added, already, missing }` + `sticky()`, `summary()`;
  `summary(name, added, already, missing)` livre.

## notify (spec 03 §4)
- `alert_command(platform, title, body)` — `notify-send -a maestria …` / `osascript -e …`.
- `Notifier::new(runner)` / `Notifier::on(runner, platform)`: `alert(title, body)` (async),
  `badge(label) -> Option<String>` — deduplicado; `Some(texto)` só no macOS = o app deve
  carimbar o dock nativamente.

## reveal (spec 03 §4)
- `reveal(runner, path)` — dbus `ShowItems` com fallback `xdg-open <pai>` / `open -R`.
- `quick_look(runner, path)` — Linux: arquivo existente + `xdg-open`, senão reveal; macOS: só o
  fallback reveal (o Quick Look nativo é do app).
- `open_link(runner, url)`, `encode_full(text)` (= `Uri.encodeFull`), `show_items_command(path)`.
