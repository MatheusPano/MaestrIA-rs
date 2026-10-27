# MaestrIA-rs

A Maestria escrita em Rust.

A Maestria é um cockpit desktop pras sessões do Claude Code: várias sessões
abertas lado a lado em painéis, cada uma no seu terminal, com o histórico, os
workspaces, o uso de contexto de cada sessão e as tarefas sugeridas que viram
uma sessão nova num clique. O que não é do núcleo vem por plugin.

A versão de hoje é em Flutter e mora em
[MaestrIA](https://github.com/MatheusPano/MaestrIA). Este repositório é o
recomeço em Rust.

## Estado

Primeiro commit. Ainda não tem código, só este README.

## Plugins

Os plugins ficam em
[maestria-plugins](https://github.com/MatheusPano/maestria-plugins). Um plugin é
uma pasta com um `maestria-plugin.json` e, quando precisa de lógica, um processo
próprio que conversa com a janela por JSON-RPC 2.0 no stdin/stdout. Como o
plugin é um processo à parte, o protocolo não depende da linguagem do app. A
referência completa está em `docs/plugins.md`, no repositório da Maestria.
