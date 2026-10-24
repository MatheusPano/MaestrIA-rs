SEED='{"folders":[{"root":$root,"name":"proj","collapsed":false}],
  "layout":{"panes":[{"folderRoot":$root,"kind":"shell","cwd":$root},
                     {"folderRoot":$root,"kind":"setup","cwd":$root,"setup":{"root":$root,"selected":"CLAUDE.md"}}],
            "tree":{"axis":"row","weights":[0.5,0.5],"children":[0,1]},"focused":0}}'

prepare() {
  printf 'linha um\n' >"$PROJECT/CLAUDE.md"
  install_probe_plugin
}

wait_file() {
  local deadline=$((SECONDS + ${2:-5}))
  while ((SECONDS < deadline)); do [[ -e $1 ]] && return 0; sleep 0.25; done
  return 1
}

scenario() {
  type_text "touch um.txt"
  key Return
  if wait_file "$PROJECT/um.txt"; then ok "typing without a click reaches the focused terminal"; else fail "the terminal did not get the keys at startup"; fi

  key F2
  key Escape
  type_text "touch dois.txt"
  key Return
  if wait_file "$PROJECT/dois.txt"; then ok "after the rename dialog closes the terminal has the keys again"; else fail "keys went nowhere after closing the rename dialog"; fi

  key ctrl+Tab
  type_text "zzz"
  key ctrl+s
  sleep 0.5
  if [[ $(head -c 3 "$PROJECT/CLAUDE.md") == zzz ]]; then ok "Ctrl+Tab moved the keys into the setup editor"; else fail "CLAUDE.md after Ctrl+Tab + typing: $(jq -Rs . <"$PROJECT/CLAUDE.md")"; fi
  if [[ -e $PROJECT/zzz ]]; then fail "the terminal also got the keys"; fi

  key ctrl+Tab
  key ctrl+alt+m
  expect_json "$PLUGIN_DATA/commands.jsonl" 'index("modal") != null' "the plugin modal opened over the panes" 8
  sleep 1
  key Escape
  type_text "touch tres.txt"
  key Return
  if wait_file "$PROJECT/tres.txt"; then ok "after the plugin modal closes the terminal has the keys again"; else fail "keys went nowhere after closing the plugin modal"; fi
}
