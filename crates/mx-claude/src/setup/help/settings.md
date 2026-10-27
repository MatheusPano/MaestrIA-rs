## O que é

O `.claude/settings.json` é lido pelo **harness** do Claude Code, não pelo modelo. Onde o `CLAUDE.md` é uma instrução que o Claude *pode* seguir, isto é uma regra que a ferramenta *faz cumprir*. Vai no git.

O `settings.local.json` é o mesmo formato, só desta máquina, e o Claude Code já o põe no `.gitignore`. É pra onde vão as permissões que você aprova clicando em "sempre permitir".

## O que dá pra configurar

- **Permissões**: `allow` e `deny` de comandos e ferramentas. `"Bash(flutter test:*)"` deixa rodar testes sem perguntar. `"Read(./.env)"` em `deny` impede de ler o arquivo.
- **Hooks**: scripts que rodam antes ou depois de uma ação, por exemplo `dart format` depois de toda edição, ou um lint que bloqueia o commit se falhar. Os eventos são `PreToolUse`, `PostToolUse`, `Stop`, `Notification` e outros.
- **Variáveis de ambiente** em `env`, pra toda sessão desta pasta.
- **Modelo padrão** e outras opções do harness.

## Exemplo

```json
{
  "permissions": {
    "allow": ["Bash(flutter test:*)", "Bash(flutter analyze:*)"],
    "deny": ["Read(./.env)"]
  },
  "hooks": {
    "PostToolUse": [{
      "matcher": "Edit|Write",
      "hooks": [{ "type": "command", "command": "dart format $CLAUDE_FILE_PATHS" }]
    }]
  }
}
```

## O .mcp.json: MCP

**MCP** é o *Model Context Protocol*: um padrão pra dar ao Claude ferramentas de fora, e contexto de fora. Um servidor MCP é um programa pequeno que expõe ferramentas (consultar um banco, abrir uma task no tracker, ler um Figma), e o Claude passa a poder chamá-las como chama `Read` ou `Bash`.

O `.mcp.json` na raiz lista os servidores **deste projeto**, e vai no git: quem clona ganha as mesmas ferramentas, e cada pessoa aprova na primeira sessão. Os seus servidores pessoais ficam em `~/.claude.json`, fora daqui.

```json
{
  "mcpServers": {
    "postgres": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-postgres", "postgresql://localhost/app"]
    },
    "wiboor": {
      "type": "http",
      "url": "https://api.wiboor.com/mcp",
      "headers": { "Authorization": "Bearer ${WIBOOR_TOKEN}" }
    }
  }
}
```

Um servidor pode ser um comando local (`command` + `args`) ou um endereço (`type: http` ou `sse`). `${VAR}` puxa da variável de ambiente, pra o token não ir pro git.
