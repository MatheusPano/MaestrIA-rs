## O que é

O `CLAUDE.md` é o texto que o Claude Code lê **no começo de toda sessão** aberta nesta pasta, antes da sua primeira mensagem. É o jeito de dizer uma vez o que você repetiria em toda conversa.

Ele fica na raiz do repo e vai no git: é do time. O `CLAUDE.md` da sua pasta pessoal (`~/.claude/CLAUDE.md`) vale pra todos os projetos; este vale só aqui, e os dois são lidos juntos. Também dá pra ter um `CLAUDE.md` dentro de uma subpasta, que só entra quando o Claude mexe em arquivos de lá.

O `CLAUDE.local.md` é o irmão que **não vai no git**: caminho de máquina, preferência sua, anotação que não é do time. Acrescente-o ao `.gitignore`.

## O que colocar

Curto e prescritivo rende mais que descritivo. O que vale:

- **Comandos exatos** de build, teste e lint. "rode `flutter test`" evita que ele descubra sozinho a cada vez.
- **Convenções** que não se deduzem do código: padrão de commit, de branch, de nome.
- **Armadilhas**: "não rode X, use Y", "o módulo Z depende de W".
- **Um mapa curto** das pastas: cinco a dez linhas sobre onde mora o quê.
- **Um ponteiro** pras decisões em `docs/adr/`, se você as escreve.

O que não vale: despejar a arquitetura inteira. Passando de umas duzentas linhas o sinal se perde no ruído, e o que importa deixa de ser lido.

## Puxar outro arquivo: o `@`

Uma linha com `@caminho` dentro do `CLAUDE.md` traz o conteúdo daquele arquivo pro contexto, como se estivesse colado ali. É o jeito de o `CLAUDE.md` ficar curto e ainda assim carregar o que importa:

```markdown
Convenções de commit: @docs/commits.md
Decisões de arquitetura: @docs/adr/README.md
```

O caminho é relativo ao `CLAUDE.md`, e vale `~/` pra pasta pessoal. Importação dentro de importação também funciona, até uns cinco níveis.

## Exemplo

```markdown
# maestria

## Comandos
- testes: /caminho/do/fvm/flutter test
- analyze: /caminho/do/fvm/flutter analyze

## Convenções
- commit: `TASK#123 add ...`, uma linha, sem corpo

## Armadilhas
- `flutter` no PATH é alias do zsh; use o binário do fvm
```
