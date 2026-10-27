SEED='{"folders":[{"root":$root,"name":"proj","collapsed":false}],
  "layout":{"panes":[{"folderRoot":$root,"kind":"shell","cwd":$root}],"tree":0,"focused":0}}'

ROW0_Y=76

scenario() {
  click 800 400
  type_text "clear; printf 'primeiro erro\\nnada aqui\\nsegundo erro\\n'"
  key Return
  sleep 1
  local before opened found closed
  before=$(shot before)
  key ctrl+shift+f
  opened=$(shot opened)
  if (($(region_diff "$before" "$opened" 880 50 400 70) > 1500)); then ok "Ctrl+Shift+F opened the find bar in the pane"; else fail "no find bar at the top right (see $opened)"; fi

  type_text "erro"
  sleep 0.5
  found=$(shot found)
  if (($(region_diff "$opened" "$found" 340 60 500 60) > 150)); then ok "typing highlighted the hits"; else fail "no highlights after typing 'erro' (see $found)"; fi
  if (($(region_diff "$opened" "$found" 880 50 400 70) > 150)); then ok "the bar shows the query and its counter"; else fail "bar unchanged after typing (see $found)"; fi

  key Return
  key shift+Return
  key Escape
  closed=$(shot closed)
  if (($(region_diff "$before" "$closed" 880 50 400 70) < 300)); then ok "Escape closed the bar"; else fail "find bar still open after Escape (see $closed)"; fi
  if (($(region_diff "$before" "$closed" 340 60 500 60) < 150)); then ok "closing removed the highlights"; else fail "highlights left behind (see $closed)"; fi

  type_text "clear; echo fim"
  key Return
  sleep 1
  tclick 600 "$ROW0_Y"
  key ctrl+shift+c
  local got
  got=$(clipboard | tr -d '[:space:]')
  if [[ $got == fim ]]; then ok "the keyboard went back to the terminal"; else fail "terminal did not get the typing after Escape (clipboard '$got')"; fi
}
