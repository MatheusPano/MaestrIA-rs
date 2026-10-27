scenario() {
  local before menu dialog after
  before=$(shot before)
  click 77 673
  menu=$(shot menu)
  if (($(region_diff "$before" "$menu" 100 560 200 130) > 2000)); then ok "footer + opened the add menu"; else fail "no add menu (see $menu)"; fi
  click 166 686
  dialog=$(shot dialog)
  if (($(region_diff "$menu" "$dialog" 380 250 520 220) > 10000)); then ok "Novo workspace opened its dialog"; else fail "no workspace dialog (see $dialog)"; fi
  type_text "ATRIUM"
  click 415 387
  click 852 435
  after=$(shot after)
  expect_config '.workspaces | length == 1' "Criar saved one workspace" 5 '.workspaces'
  expect_config '.workspaces[0].name == "ATRIUM"' "workspace named ATRIUM" 2 '.workspaces'
  expect_config "(.workspaces[0].folderRoots // []) | index(\"$PROJECT\") != null" "the checked folder belongs to it" 2 '.workspaces'
  if (($(region_diff "$dialog" "$after" 380 250 520 220) > 10000)); then ok "dialog closed"; else fail "dialog still open (see $after)"; fi
  if (($(region_diff "$before" "$after" 30 60 370 300) > 500)); then ok "sidebar shows the workspace"; else fail "sidebar unchanged (see $after)"; fi
}
