SEED='{"folders":[{"root":$root,"name":"proj","collapsed":false}],
  "layout":{"panes":[{"folderRoot":$root,"kind":"shell","cwd":$root},{"folderRoot":$root,"kind":"shell","cwd":$root,"hotkey":"alt+2"}],
            "tree":{"axis":"row","weights":[0.5,0.5],"children":[0,1]},"focused":0}}'

scenario() {
  local before hidden shown typed zoomed
  click 620 400
  before=$(shot before)
  expect_config '.layout.focused == 0' "left pane focused" 3 '.layout.focused'

  key ctrl+shift+b
  hidden=$(shot hidden)
  expect_config '.sidebarHidden == true' "Ctrl+Shift+B from the terminal hid the sidebar" 5 '.sidebarHidden'
  if (($(region_diff "$before" "$hidden" 60 60 300 400) > 5000)); then ok "sidebar gone from the screen"; else fail "sidebar still drawn (see $hidden)"; fi
  key ctrl+shift+b
  shown=$(shot shown)
  expect_config '(.sidebarHidden // false) == false' "Ctrl+Shift+B again brought it back" 5 '.sidebarHidden'
  if (($(region_diff "$before" "$shown" 60 60 300 400) < 2000)); then ok "sidebar drawn again"; else fail "sidebar not restored (see $shown)"; fi

  expect_config '.layout.panes[1].hotkey == "alt+2" and (.layout.panes[0].hotkey == null)' "the panel hotkey survives the round trip" 5 '.layout.panes'
  key alt+2
  expect_config '.layout.focused == 1' "Alt+2 brought the panel that owns it" 5 '.layout.focused'
  type_text "clear; echo slot-two"
  key Return
  sleep 1
  typed=$(shot typed)
  if (($(region_diff "$shown" "$typed" 880 60 380 80) > 300)); then ok "typing after Alt+2 lands in the right pane"; else fail "right pane unchanged after typing (see $typed)"; fi
  if (($(region_diff "$shown" "$typed" 470 66 330 28) < 30)); then ok "left pane got no text"; else fail "left pane changed after Alt+2 + typing (see $typed)"; fi

  key ctrl+equal
  sleep 0.5
  zoomed=$(shot zoomed)
  expect_config '.layout.panes[1].zoom == 1' "Ctrl+= zoomed the focused pane" 5 '.layout.panes'
  expect_config '(.layout.panes[0].zoom // 0) == 0' "the other pane kept its zoom" 2 '.layout.panes'
  if (($(region_diff "$typed" "$zoomed" 880 60 380 80) > 300)); then ok "right pane text redrawn bigger"; else fail "right pane looks the same after Ctrl+= (see $zoomed)"; fi
}
