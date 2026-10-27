## O que é

Uma skill é um **fluxo com nome**, que você invoca digitando `/nome` na sessão, ou que o Claude escolhe sozinho quando o seu pedido bate na descrição dela. Cada uma é uma pasta em `.claude/skills/` com um `SKILL.md` dentro; o cabeçalho diz o nome e quando ela vale, o corpo diz o passo a passo.

A diferença pra um parágrafo no `CLAUDE.md`: a skill só é lida quando chamada, então pode ser longa e detalhada sem custar contexto nas outras conversas. E pode levar arquivos junto na mesma pasta: um script, um modelo, uma referência.

## O frontmatter

O bloco entre os dois `---` no topo do `SKILL.md` é o **frontmatter**: um cabeçalho em YAML que o Claude Code lê pra saber o que a skill é *sem* ler o corpo dela. É por ele que a skill aparece no `/` e é escolhida sozinha.

- `name`: o nome depois da barra. Minúsculas e hífens.
- `description`: **quando** usar. É o texto que o Claude compara com o seu pedido pra decidir invocar a skill por conta própria; escreva os gatilhos ("use quando o pedido for fechar versão, gerar DMG ou publicar").
- `allowed-tools` (opcional): as ferramentas que a skill pode usar sem perguntar, como `Bash(git *)`.
- `disable-model-invocation` (opcional): `true` deixa a skill só pro `/nome` digitado, sem o Claude escolhê-la sozinho. Pra o que não pode rodar sem você pedir.

## Quando usar

Tudo que você faz repetidas vezes e sempre do mesmo jeito:

- gerar a release: bump de versão, changelog, tag, DMG;
- subir o app numa VM pra reproduzir um bug;
- fazer a revisão de um PR com o checklist do time;
- montar um relatório com formato fixo.

## Exemplo

`.claude/skills/release/SKILL.md`:

```markdown
---
name: release
description: Gera uma release do maestria. Use quando o pedido for fechar versão, gerar DMG ou publicar.
---

# Release

1. Confirme que `flutter analyze` e `flutter test` passam.
2. Suba a versão em `pubspec.yaml` e peça o número novo se não foi dito.
3. Rode `packaging/macos/make-dmg.sh`.
4. Commit em uma linha, no padrão do time, e tag `vX.Y.Z`.
```

Os `.md` soltos em `.claude/commands/` são o formato antigo da mesma ideia: o corpo é o prompt que `/nome` dispara. Ainda funcionam; skills fazem o mesmo com mais estrutura.
