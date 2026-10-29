SEED='{"theme":"ayu-dark","folders":[{"root":$root,"name":"proj","collapsed":false}],
  "layout":{"panes":[{"folderRoot":$root,"kind":"shell","cwd":$root}],"tree":0,"focused":0}}'

ROW0_Y=76

scenario() {
  click 800 400
  type_text "clear; printf '\\033[41m%40s\\033[0m\\n' ''"
  key Return
  sleep 1
  local block px
  block=$(shot block)
  px=$(pixel "$block" 500 "$ROW0_Y")
  if [[ ${px,,} == ea6c73 ]]; then ok "terminal uses the saved theme's red on start ($px)"; else fail "expected ayu-dark red ea6c73 at row 0, got $px (see $block)"; fi
}
