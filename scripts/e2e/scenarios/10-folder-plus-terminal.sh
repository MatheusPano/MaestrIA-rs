scenario() {
  local before menu after
  before=$(shot before)
  if wait_config '.layout.panes | length == 0' 1; then ok "starts with no panes"; else fail "expected no panes at start"; fi

  click 352 90
  menu=$(shot menu)
  if (($(region_diff "$before" "$menu" 360 100 200 250) > 2000)); then ok "+ opened the menu"; else fail "+ did not open a menu (see $menu)"; fi

  click 415 149
  sleep 1.5
  after=$(shot terminal)
  expect_config '.layout.panes | length == 1 and .[0].kind == "shell"' "Terminal opened a shell pane in config.json" 5 '.layout'
  if (($(region_diff "$before" "$after" 420 10 840 40) > 500)); then ok "pane header rendered"; else fail "no pane header after Terminal (see $after)"; fi
  if (($(region_diff "$before" "$after" 60 130 330 40) > 500)); then ok "sidebar shows the new session row"; else fail "no session row in the sidebar (see $after)"; fi
}
