#!/bin/sh
idof() { echo "$1" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p'; }
while IFS= read -r line; do
  case "$line" in
    *'"method":"initialize"'*)
      echo "{\"jsonrpc\":\"2.0\",\"id\":$(idof "$line"),\"result\":{}}" ;;
    *'"method":"suggestion.start"'*)
      echo "{\"jsonrpc\":\"2.0\",\"id\":$(idof "$line"),\"result\":null}"
      echo '{"jsonrpc":"2.0","id":"p1","method":"suggestion.created","params":{"suggestionId":"task_00000001","ref":"TASK#1","intro":"Vamos trabalhar na TASK#1","branch":"feature/TASK#1","dir":"TASK-1"}}' ;;
    *'"method":"shutdown"'*)
      exit 0 ;;
  esac
done
