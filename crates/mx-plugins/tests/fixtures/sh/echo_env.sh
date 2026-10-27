#!/bin/sh
while IFS= read -r line; do
  case "$line" in
    *'"method":"initialize"'*)
      echo "$line" > "$MAESTRIA_PLUGIN_DATA/initialize.json"
      printf '%s|%s|%s|%s\n' "$MAESTRIA_API" "$MAESTRIA_PLUGIN_ID" "$MAESTRIA_PLUGIN_DIR" "$MAESTRIA_PLUGIN_DATA" > "$MAESTRIA_PLUGIN_DATA/env.txt"
      id=$(echo "$line" | sed 's/.*"id":\([0-9]*\).*/\1/')
      echo "{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{}}" ;;
  esac
done
