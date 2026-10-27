SEED='{"folders":[{"root":$root,"name":"proj","collapsed":false}],
  "layout":{"panes":[{"folderRoot":$root,"kind":"shell","cwd":$root}],"tree":0,"focused":0}}'

prepare() {
  mkdir -p "$MAESTRIA_HOME/plugins"
  cp -R "$HERE/fixtures/keys-plugin" "$MAESTRIA_HOME/plugins/e2e.keys"
  PLUGIN_DATA=$MAESTRIA_HOME/plugin-data/e2e.keys
}

scenario() {
  click 800 400
  type_text "clear"
  key Return
  printf 'echo colado' | on_display xclip -selection clipboard
  key ctrl+alt+k
  expect_json "$PLUGIN_DATA/calls.jsonl" 'any(.method == "session.rename")' "the plugin ran the sessions API" 10
  expect_json "$PLUGIN_DATA/calls.jsonl" '[.[] | select(.error != null)] | length == 0' "no call was refused" 2
  expect_json "$PLUGIN_DATA/calls.jsonl" \
    'any(.method == "initialize" and .params.keys == 1 and .params.rename == 1 and .params.closeSessions == 1)' \
    "initialize advertised the sessions API flags" 2
  local deadline=$((SECONDS + 6)) lines=0
  while ((SECONDS < deadline)); do
    lines=$(grep -c '^colado$' "$PLUGIN_DATA/out.txt" 2>/dev/null)
    ((lines >= 2)) && break
    sleep 0.25
  done
  if ((lines == 2)); then ok "the pasted clipboard ran, and up + enter ran it again"; else fail "out.txt has $lines 'colado' lines"; fi
  expect_json "$PLUGIN_DATA/calls.jsonl" 'any(.method == "session.rename" and .result.title == "renomeado pelo plugin")' \
    "session.rename trimmed the title" 2
  expect_config 'any(.. | objects; .label? == "renomeado pelo plugin")' "the new name was saved" 5
}
