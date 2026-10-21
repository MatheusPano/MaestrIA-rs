## O que é

Uma regra é um pedaço de `CLAUDE.md` que **só entra no contexto quando é preciso**. Cada `.md` em `.claude/rules/` pode dizer, no cabeçalho, a quais arquivos ele se aplica; quando o Claude lê ou edita um arquivo que bate no glob, a regra é carregada. Sem `paths`, ela entra sempre, como se fosse parte do `CLAUDE.md`.

É o que mantém o `CLAUDE.md` enxuto: o que só importa em `test/` fica num arquivo que só aparece quando se está em `test/`.

## O frontmatter

O bloco entre os dois `---` no topo do arquivo é o **frontmatter**: um cabeçalho em YAML com dados *sobre* o arquivo, que o Claude Code lê antes do texto. Aqui ele tem um campo só:

- `paths`: lista de globs. `test/**` é tudo dentro de `test/`; `lib/ui/*.dart` é só o primeiro nível de `lib/ui/`. A regra entra quando um arquivo que bate em algum deles é lido ou editado.

Sem frontmatter, a regra vale sempre.

## Quando usar

- Convenções de uma parte só do repo: como escrever um teste, como é um widget aqui, o que um migration precisa ter.
- Um `CLAUDE.md` que passou de duzentas linhas e quer ser repartido.
- Instruções que se organizam em subpastas, que o Claude Code lê do mesmo jeito.

## Exemplo

`.claude/rules/tests.md`:

```markdown
---
paths:
  - "test/**"
---

# Testes

- Um `group` por comportamento, nomes em português, na primeira pessoa do que o código faz.
- Nada de `pumpAndSettle` com animação infinita: use `pump` com duração.
- Store sob teste sempre com `addTearDown(store.dispose)`.
```
