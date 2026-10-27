#!/bin/sh
# A linha de status de uma sessão aberta pela maestria: manda o que o
# Claude Code mostrou pra janela e devolve a linha de status do usuário.
url="$1"
user="$2"
input=$(cat)
# Em segundo plano e com a saída descartada: a linha de status não pode
# esperar pela janela, nem quando ela está fechada -- nem quebrar quando
# não há curl.
{ printf '%s' "$input" | curl -s -m 1 -X POST -H 'Content-Type: application/json' \
  --data-binary @- "$url"; } >/dev/null 2>&1 &
if [ -n "$user" ]; then
  # `--decode` é o que o Linux e o macOS de hoje entendem; o `-D` é a volta
  # do macOS antigo.
  cmd=$(printf '%s' "$user" | base64 --decode 2>/dev/null || printf '%s' "$user" | base64 -D)
  printf '%s' "$input" | sh -c "$cmd"
fi
