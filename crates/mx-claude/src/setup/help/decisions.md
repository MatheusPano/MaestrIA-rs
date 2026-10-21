## O que é

Um ADR (*Architecture Decision Record*) é **uma decisão de arquitetura por arquivo**: o contexto em que foi tomada, o que se decidiu, e o que isso custa. Fica em `docs/adr/`, numerado, e nunca é reescrito: uma decisão que muda vira um ADR novo que substitui o antigo.

Não é um formato do Claude Code, é um formato de time. Mas ajuda o Claude por um motivo específico: a decisão **contra-intuitiva**. "Por que o terminal tem um VT próprio em vez de usar a lib X?" é exatamente o tipo de coisa que uma sessão nova tenderia a "corrigir" sem saber. Com o ADR, ela lê o porquê antes.

## O detalhe que importa

O Claude **não lê esta pasta sozinho**. É o `CLAUDE.md` que tem de apontar pra ela: uma linha como "as decisões de arquitetura estão em `docs/adr/`; leia antes de mudar algo estrutural". Sem isso, os ADRs são documentação pra gente, e só.

## Exemplo

`docs/adr/0003-vt-proprio.md`:

```markdown
# ADR 0003: parser VT próprio em vez do do xterm

- **Status:** aceita
- **Data:** 2026-03-10

## Contexto
O xterm.dart perde a cor de fundo em sequências OSC compostas, e o
Claude Code as usa no cabeçalho.

## Decisão
Um parser próprio em `services/vt.dart`, só pro subconjunto que o
Claude Code emite.

## Consequências
Menos dependência, mais código nosso. Toda sequência nova precisa de
um teste em `vt_test.dart`.
```
