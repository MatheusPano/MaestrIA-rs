#!/bin/sh
idof() { echo "$1" | sed 's/.*"id":\([0-9]*\).*/\1/'; }
while IFS= read -r line; do
  case "$line" in
    *'"method":"initialize"'*)
      echo "{\"jsonrpc\":\"2.0\",\"id\":$(idof "$line"),\"result\":{}}" ;;
    *'"method":"command.invoke"'*)
      echo '{"jsonrpc":"2.0","id":"p1","method":"window.showBanner","params":{"text":"oi"}}'
      echo 'isto não é json'
      echo "{\"jsonrpc\":\"2.0\",\"id\":$(idof "$line"),\"result\":\"feito\"}" ;;
    *'"method":"event"'*)
      echo '{"jsonrpc":"2.0","method":"log","params":{"message":"evento"}}' ;;
    *'"method":"shutdown"'*)
      exit 0 ;;
  esac
done
