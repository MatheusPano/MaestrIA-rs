# Regras para agentes implementadores

1. Leia `docs/BLUEPRINT.md` e a(s) spec(s) do seu crate em `docs/spec/`. A spec é a
   fonte da verdade do comportamento; o Dart em `/home/lucas/maestria/repo` é só
   referência de leitura (nunca modifique).
2. Você é dono **apenas** do(s) crate(s) atribuído(s). Não edite outros crates nem o
   `Cargo.toml` raiz; use `foo.workspace = true` para deps já declaradas lá. Se
   precisar de uma dep nova, adicione-a só no `Cargo.toml` do seu crate com versão
   explícita.
3. Build isolado: `export CARGO_TARGET_DIR=target-<seu-crate> CARGO_BUILD_JOBS=8` (máquina livre)
   e sempre `cargo <cmd> -p <seu-crate>` — nunca `--workspace` (outros crates estão em
   obras). Feche com `cargo test -p <crate>` e
   `cargo clippy -p <crate> --all-targets -- -D warnings` limpos.
4. `#![forbid(unsafe_code)]`. Arquivos ≤ 400 LOC (teto 600).
5. **Comentários: o padrão é zero.** Nada de descrever o que o código faz, narrar
   decisão ou comentar teste. Só uma linha quando há um gotcha externo não
   descobrível pelo código. Sem doc-comments decorativos.
6. Portar **todos** os casos given/when/then da spec que caem no seu crate como testes
   Rust (o nome do teste é a frase). `proptest` onde a spec/blueprint indicar.
7. **Nunca leia nem escreva `~/.maestria`** nem `~/.claude` reais — o app Flutter está
   rodando. Caminhos de estado vêm de um parâmetro/`MAESTRIA_HOME`; testes usam
   `tempfile`.
8. Não faça `git commit`. Não mate processos que você não iniciou.
9. API pública enxuta e estável: outros crates (store, app) vão consumi-la. Ao
   terminar, escreva `crates/<crate>/API.md` (≤ 80 linhas) listando tipos/funções
   públicos e o que cada um faz — é o contrato para a próxima onda.
10. Recortes de string contam unidades UTF-16 como o Dart (use `mx_core::text`).
