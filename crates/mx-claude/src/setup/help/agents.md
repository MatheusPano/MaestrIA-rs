## O que é

Um agente (ou **subagente**) é um segundo Claude que a sessão principal chama pra fazer uma tarefa e voltar com o resultado. Ele roda **com o próprio contexto**, o próprio prompt de sistema e a própria lista de ferramentas; a conversa principal não vê o que ele leu, só o que ele respondeu.

Isso serve pra duas coisas:

- **Não sujar o contexto.** Uma busca que abre trinta arquivos pra achar um deles enche a conversa principal de código que não vai ser usado. Feita por um agente, só a resposta volta.
- **Especializar.** Um revisor que só pode ler, com um checklist fixo, revisa melhor que a mesma sessão que acabou de escrever o código.

O Claude Code já vem com alguns (o explorador, o planejador). Os seus ficam em `.claude/agents/`, um `.md` por agente, e a sessão principal os usa quando o pedido bate na descrição, ou quando você pede pelo nome.

## O frontmatter

O bloco entre os dois `---` no topo é o **frontmatter**: o cabeçalho em YAML com os dados do agente, que a sessão principal lê pra decidir quando e como chamá-lo.

- `name`: como ele se chama.
- `description`: **quando** delegar pra ele. É o que a sessão principal lê pra decidir usá-lo; escreva como uma frase de gatilho.
- `tools`: o que ele pode usar. Um revisor sem `Edit` e sem `Write` não consegue "consertar" o que devia só apontar.
- `model`: `inherit` usa o da sessão; `haiku` deixa uma busca simples mais barata.

O corpo é o prompt de sistema dele.

## Exemplo

`.claude/agents/revisor.md`:

```markdown
---
name: revisor
description: Revisa um diff ou PR procurando bug, quebra de convenção e teste faltando. Use ao pedir revisão, code review ou "olha isso antes de eu commitar".
tools: Read, Grep, Glob, Bash
model: inherit
---

Você revisa código do maestria. Não edita nada: aponta.

Pra cada achado diga arquivo e linha, o problema em uma frase, e como reproduzir se for bug. Ordene do mais grave pro mais leve. Cheque: null safety, timers sem cancel, setState depois de dispose, e se toda mudança de comportamento tem teste.
```
