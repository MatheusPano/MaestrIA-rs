SEED='{"folders":[{"root":$root,"name":"proj","collapsed":false}],
  "layout":{"panes":[{"folderRoot":$root,"kind":"shell","cwd":$root}],"tree":0,"focused":0}}'

scenario() {
  local before dialog after
  click 800 400
  before=$(shot before)
  key F2
  dialog=$(shot dialog)
  if (($(region_diff "$before" "$dialog" 500 260 280 200) > 5000)); then ok "F2 opened the rename dialog"; else fail "no dialog after F2 (see $dialog)"; fi
  key ctrl+a BackSpace
  type_text "renomeado"
  key Return
  after=$(shot after)
  expect_config '.layout.panes[0].label == "renomeado"' "Enter saved the new title as the pane label" 5 '.layout.panes'
  if (($(region_diff "$dialog" "$after" 500 260 280 200) > 5000)); then ok "dialog closed"; else fail "dialog still open after Enter (see $after)"; fi
  if (($(region_diff "$before" "$after" 420 10 300 30) > 100)); then ok "pane header shows the new title"; else fail "pane header unchanged (see $after)"; fi
  if (($(region_diff "$before" "$after" 60 120 300 60) > 100)); then ok "sidebar row shows the new title"; else fail "sidebar row unchanged (see $after)"; fi
}
