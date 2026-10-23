SEED='{"folders":[{"root":$root,"name":"proj","collapsed":false}],
  "layout":{"panes":[{"folderRoot":$root,"kind":"shell","cwd":$root}],"tree":0,"focused":0}}'

ROW0_Y=76

scenario() {
  click 800 400
  type_text "clear; echo ok"
  key Return
  sleep 1
  local out
  out=$(shot echo)
  tclick 600 "$ROW0_Y"
  key ctrl+shift+c
  local got
  got=$(clipboard | tr -d '[:space:]')
  if [[ $got == ok ]]; then ok "pane row 0 reads back 'ok' via select + Ctrl+Shift+C"; else fail "clipboard after selecting row 0 is '$got' (see $out)"; fi

  click 800 400
  type_text "clear; printf '\\033[42m%40s\\033[0m\\n' ''"
  key Return
  sleep 1
  local block px
  block=$(shot block)
  px=$(pixel "$block" 500 "$ROW0_Y")
  if python3 -c "import sys;h=sys.argv[1];r,g,b=(int(h[i:i+2],16) for i in (0,2,4));sys.exit(0 if g>r+40 and g>b+30 else 1)" "$px"; then
    ok "SGR 42 painted a green cell ($px)"
  else
    fail "expected green bg at row 0, got $px (see $block)"
  fi
}
