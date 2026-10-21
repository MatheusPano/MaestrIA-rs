# Maestria em Rust — Blueprint

Fonte da verdade do comportamento: `docs/spec/01..05`. Este documento decide a
arquitetura, a divisão do trabalho e os critérios de pronto.

## 1. Censo do legado e bugs latentes

| métrica (lib/) | valor |
|---|---|
| arquivos Dart | 51 |
| LOC de código / comentário | 26 273 / 8 364 (31,8%) |
| maior arquivo | 4 773 LOC (`store.dart`) |
| arquivos ≤ 400 LOC | 52,9% |
| testes | 58 arquivos, ~600 casos (reescritos como given/when/then nas specs) |
| runtime vivo (2.7.0) | **944 MB RSS (pico 1074 MB), 169 threads, 10,1% CPU médio** |

Bugs latentes que o Rust corrige de propósito (registrados na telemetria):

1. Um campo de tipo errado em workspace/grupo/launcher aborta o load do `config.json`
   e o próximo save grava o estado parcial por cima. Rust: pula só a entrada ruim.
2. `session_id`/`permission_mode` não-string derruba o reducer de hooks. Rust: ausente.
3. Alt+1…9 no Linux vaza `ESC dígito` pro programa. Rust: intercepta.
4. `TERM` herdado do terminal que abriu o app. Rust: `TERM=xterm-256color` explícito.
5. Roda do mouse mapeava célula em coordenada de janela. Rust: coordenada local do painel.

## 2. Arquitetura alvo

Workspace Cargo, um crate por fronteira, todo crate de biblioteca com
`#![forbid(unsafe_code)]`, arquivos ≤ 400 LOC (teto 600).

```
mx-theme    paletas (19 temas), JSON de tema, 13 tints, tipografia          puro
mx-core     modelo, árvore de painéis, config.json (ler/gravar/migrar),
            reducer de hooks, texto UTF-16, ports (traits)                   puro
mx-sys      shell/login env, git, editor, histórico ~/.claude, .code-workspace,
            `claude agents --json`, notificações, abrir/revelar arquivo      tokio
mx-term     PTY + alacritty_terminal, scrubber de VT, encoders teclado/mouse,
            IME guard, seleção/cópia, links (regex + OSC 8)                  threads
mx-claude   servidor HTTP loopback (/hook, /status, /mcp), MCP maestria,
            statusline.sh, uso do plano (oauth/usage), setup pane model,
            docs/leitor (modelo)                                             tokio+axum
mx-plugins  manifesto, instalar/atualizar/dev-link, host JSON-RPC 2.0 stdio,
            parser RFW texto + modelo de widgets, floats/modais/comandos    tokio
mx-store    o AppStore: estado único dirigido por comandos, timers via Clock,
            liga core+sys+term+claude+plugins por traits                     tokio
mx-app      binário `maestria` (gpui): janela, sidebar, rail, painéis,
            TerminalView, leitor markdown, menus, diálogos, settings,
            renderer RFW, animações, empacotamento                           gpui
```

Grafo: `theme ← core ← {sys, term, claude, plugins} ← store ← app`. `term`,
`claude` e `plugins` não se conhecem; o store os costura.

Concorrência: o store é dono único do estado (sem locks finos). Roda na thread de UI
do gpui como `Entity<Store>`; I/O (PTY, HTTP, plugins, git) roda em tasks/threads que
devolvem mensagens ao store. Isso substitui o isolate único do Dart sem 169 threads.

Mapeamento legado → Rust: ver a seção "Rust layout" de cada spec (01 §6, 02 §6,
03 §8, 04 §7, 05 §7). Ajustes: o `maestria-core` da spec 01 é dividido em
`mx-core` (model/layout/persist) + `mx-sys` (sys/) + `mx-store` (store/).

## 3. Compatibilidade (inegociável)

- `~/.maestria/config.json` lido e gravado byte-compatível na ordem de chaves
  (`serde_json` + `preserve_order`), com todas as migrações legadas.
- `~/.maestria/statusline.sh` byte-idêntico; `hook-port` reutilizado.
- Argv do `claude` idêntico (`--mcp-config` primeiro), JSON de `--settings` idêntico.
- MCP `suggest_task`/`dismiss_task` wire-idênticos.
- Plugins existentes (`maestria-plugins/*`, `argocd-plugin`) rodam sem mudança:
  manifesto, JSON-RPC e subconjunto RFW usado.
- Recortes de string contam unidades UTF-16, como o Dart.
- Diretório de estado configurável por `MAESTRIA_HOME` (padrão `~/.maestria`).
  **Desenvolvimento e testes nunca usam `~/.maestria`** — o app Flutter está vivo.

## 4. Testes e paridade

- Cada spec traz tabelas given/when/then portadas do Dart; cada uma vira teste Rust no
  crate dono. Meta: todos os casos não-[UI] portados e passando.
- `proptest` onde há invariantes: árvore de painéis (insert/remove preserva ordem e
  conjunto), round-trip do config, scrubber de VT (nunca corta sequências válidas),
  clip UTF-16, parser RFW (round-trip).
- Paridade A/B (substitui "seeds" numéricas da skill): fixtures reais —
  `test/fixtures/config-2.4.0.json`, `history/**`, payloads de hook, mensagens MCP,
  manifestos dos plugins reais — passados pelos dois e comparados.
- `cargo clippy --workspace --all-targets -D warnings` limpo.

## 5. Objetivo do usuário: desempenho sob carga

O problema real: com muitas sessões ligadas ao mesmo tempo o app fica lento e
**duplica letras ao digitar** (repetição de caracteres vista ao vivo no 2.7.0 — o
"ibus triple commit" da spec 02 §3.5 somado a input atrasado atrás de quadros lentos).
Relato do Lucas: acontece ao digitar rápido com CPU alta, com qualquer caractere (não só
acentos). Ambiente: X11, `ibus-daemon --xim`, `XMODIFIERS=@im=ibus`, GTK sem
`GTK_IM_MODULE` (usa ibus). Hipótese principal: o ibus em modo assíncrono estoura o
tempo de resposta sob carga, o GTK trata a tecla como não-tratada e o app a insere
crua; depois o commit do ibus chega e a mesma letra entra de novo. Teste de aceitação
A/B: `stress-ng --cpu 16` + `xdotool type --delay 15` de um texto de 2 000 caracteres
num `cat > arquivo` dentro de cada app; contar divergências. Meta Rust: 0.

Critérios de pronto, acima de qualquer outra métrica:

- Nenhuma letra duplicada: o caminho tecla → PTY é síncrono no handler de input do
  gpui, sem passar pelo store nem por quadro; o guard de IME é portado e testado
  (commit repetido do ibus, composição pt-BR com acentos mortos).
- Latência tecla → eco < 16 ms com 12 sessões do Claude produzindo saída ao mesmo tempo.
- Saída de um painel nunca bloqueia outro: cada PTY lê numa thread própria e só marca
  "sujo"; o redraw é coalescido a 1 quadro e só dos painéis visíveis; painéis ocultos
  não renderizam nada.
- CPU ociosa com 12 sessões paradas ≈ 0 (sem ticks redesenhando a janela).

## 6. Benchmarks (escada de 4 degraus da skill)

Métricas de app desktop em vez de seeds: RSS ocioso com 6 sessões, threads, CPU
ociosa (ms CPU por segundo), tempo até a primeira janela, throughput do terminal
(`cat` de 50 MB). Degraus: `legacy_baseline` (medido: 944 MB / 101 ms/s) →
`safe_rust` → `soa_simd` (grid snapshot sem alocação por quadro, runs de bg
agrupados, cache de glyph) → `pgo_lto` (perfil de uso real + fat LTO).

## 7. Roteiro de execução

Onda 0 — spike gpui + alacritty (em andamento): workspace, TerminalView.

Onda 1 — paralela, um agente por crate, cada um com seu `CARGO_TARGET_DIR`:
`mx-theme`, `mx-core`, `mx-sys`, `mx-term`, `mx-claude`, `mx-plugins`.

Onda 2 — `mx-store` (2 agentes: estado/abas/painéis; fluxos/sugestões/grupos/hibernação)
e, em paralelo, `mx-app` em fatias (shell da janela + sidebar; painéis + terminal;
leitor markdown + setup; settings + diálogos + menus; renderer RFW + telas de plugin).

Onda 3 — integração, rodar o app de verdade com `MAESTRIA_HOME` isolado, paridade,
clippy, benchmarks, PGO, empacotamento `.deb`, infográfico.

Questão em aberto (de produto, não bloqueia): ditado está comentado no 2.4 — fica fora.
