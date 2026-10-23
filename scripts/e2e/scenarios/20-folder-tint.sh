scenario() {
  local before menu sub after
  before=$(shot before)
  rclick 140 85
  menu=$(shot menu)
  if (($(region_diff "$before" "$menu" 140 100 200 350) > 2000)); then ok "right-click opened the folder menu"; else fail "no context menu (see $menu)"; fi

  move 190 450
  sleep 0.8
  sub=$(shot cor)
  if (($(region_diff "$menu" "$sub" 420 200 180 400) > 2000)); then ok "Cor opened the tint submenu"; else fail "Cor submenu did not open (see $sub)"; fi

  click 416 256
  after=$(shot after)
  expect_config '.folders[0].tint == "red"' "Vermelho saved as folders[0].tint" 5 '.folders'
  if (($(region_diff "$before" "$after" 86 78 26 26) > 20)); then ok "folder icon repainted"; else fail "folder icon unchanged after tint (see $after)"; fi
}
