#!/bin/sh
idof() { echo "$1" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p'; }
tabof() { echo "$1" | sed -n 's/.*"tabId":"\([^"]*\)".*/\1/p'; }
while IFS= read -r line; do
  case "$line" in
    *'"method":"initialize"'*)
      echo "{\"jsonrpc\":\"2.0\",\"id\":$(idof "$line"),\"result\":{}}" ;;
    *'"method":"tool.call"'*)
      echo "{\"jsonrpc\":\"2.0\",\"id\":$(idof "$line"),\"result\":{\"content\":[{\"type\":\"text\",\"text\":\"ok $(tabof "$line")\"}]}}" ;;
    *'"method":"shutdown"'*)
      exit 0 ;;
  esac
done
