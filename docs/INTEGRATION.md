# Onda de integração — donos de arquivo

Todos em `crates/mx-app` com `CARGO_TARGET_DIR=target-app` (o lock do cargo serializa).
Edite só o que é seu; para o resto, peça via a API pública do dono ou deixe um ponto
de encaixe pequeno e avise no relatório.

| agente | dono de |
|---|---|
| wire-chrome | `src/app/` (exceto `keys.rs`), `src/sidebar/`, `src/status_bar/`, `src/chrome/`, `main.rs` |
| panes-plugins | `src/panes/`, `src/rail/`, `src/rfw_view/`, `src/reader/`, novo `src/plugin_ui/`, `crates/mx-store/src/plugins*` |
| keymap-term | novo `src/keymap/`, `src/app/keys.rs`, `src/terminal_*.rs`, `crates/mx-term` |
| setup-ui | novo `src/setup/` |

Pontos de encaixe combinados:
- `setup-ui` expõe `setup::SetupView::new(...)`; `panes-plugins` embute no painel de setup.
- `keymap-term` expõe `keymap::check_chord(...) -> ChordCheck` para o gravador de atalhos
  do settings e `keymap::Action` para o root despachar; `wire-chrome` consome.
- Menus de contexto do terminal usam os builders de `chrome::menu` (`keymap-term` chama,
  não edita).
- Rodar o app sempre com `MAESTRIA_HOME` e `HOME` apontando para uma pasta no scratchpad.
