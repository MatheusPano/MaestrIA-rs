SEED='{"folders":[{"root":$root,"name":"proj","collapsed":false}],
  "layout":{"panes":[{"folderRoot":$root,"kind":"claude","cwd":$root}],"tree":0,"focused":0}}'

prepare() {
  mkdir -p "$HOME/.local/bin"
  cat >"$HOME/.local/bin/claude" <<SH
#!/bin/sh
[ -t 0 ] || exit 0
printf 'fake claude pronto\r\n'
while :; do date +%s%N >"$E2E_TMP/claude.beat"; sleep 0.2; done
SH
  chmod +x "$HOME/.local/bin/claude"
}

claude_alive() {
  local a b
  a=$(cat "$E2E_TMP/claude.beat" 2>/dev/null) || return 1
  sleep 1
  b=$(cat "$E2E_TMP/claude.beat" 2>/dev/null)
  [[ -n $b && $a != "$b" ]]
}

one_claude() { echo ".layout.panes | length == $1 and (map(select(.kind == \"claude\")) | length == 1)"; }

scenario() {
  sleep 3
  click 800 400
  if claude_alive; then ok "fake claude is running"; else fail "fake claude never started"; fi

  key ctrl+shift+Return
  sleep 1.5
  shot shortcut >/dev/null
  expect_config "$(one_claude 2)" "Ctrl+Shift+Enter opened a shell and kept the claude tab" 5 '.layout'
  if claude_alive; then ok "claude survived the new terminal"; else fail "claude process was killed by Ctrl+Shift+Enter"; fi

  click 140 155
  sleep 1
  click 352 90
  shot menu >/dev/null
  click 372 149
  sleep 1.5
  shot plus >/dev/null
  expect_config "$(one_claude 3)" "+ > Terminal over the claude row kept the claude tab" 5 '.layout'
  if claude_alive; then ok "claude survived + > Terminal"; else fail "claude process was killed by + > Terminal"; fi

  move 140 155
  click 375 155
  sleep 1.5
  expect_config '.layout.panes | length == 2 and all(.kind == "shell")' "the row's x still closes the claude tab" 5 '.layout'
  sleep 3
  if claude_alive; then fail "claude kept running after its tab was closed"; else ok "closing the tab stopped claude"; fi
}
