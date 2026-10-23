scenario() {
  local before dialog picked after
  before=$(shot before)
  click 377 30
  sleep 0.8
  dialog=$(shot dialog)
  if (($(region_diff "$before" "$dialog" 520 100 640 500) > 20000)); then ok "gear opened settings"; else fail "settings did not open (see $dialog)"; fi

  click 724 210
  sleep 0.8
  picked=$(shot picked)
  click 986 642
  sleep 0.8
  after=$(shot after)
  expect_config '.theme == "ayu-dark"' "theme saved as ayu-dark" 5 '.theme'
  local bg0 bg1
  bg0=$(pixel "$before" 800 600)
  bg1=$(pixel "$after" 800 600)
  if [[ $bg0 != "$bg1" ]]; then ok "pane background changed $bg0 -> $bg1"; else fail "pane background still $bg0 after switching theme (see $after)"; fi
  if (($(region_diff "$dialog" "$after" 760 380 400 300) > 20000)) && [[ $(pixel "$after" 986 642) == "$(pixel "$after" 1000 600)" ]]; then ok "settings closed with Pronto"; else fail "settings still open after Pronto (see $after)"; fi
}
