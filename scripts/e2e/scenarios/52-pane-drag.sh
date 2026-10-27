SEED='{"folders":[{"root":$root,"name":"proj","collapsed":false}],"sidebarHidden":true,
  "layout":{"panes":[{"folderRoot":$root,"kind":"shell","cwd":$root,"label":"um"},{"folderRoot":$root,"kind":"shell","cwd":$root,"label":"dois"}],
            "tree":{"axis":"row","weights":[0.5,0.5],"children":[0,1]},"focused":0}}'

scenario() {
  local before after
  before=$(shot before)
  drag 1000 22 110 330 16
  after=$(shot after)
  expect_config '.layout.tree.children == [1,0]' "dropping dois by its header on the left edge of um put it first" 5 '.layout.tree'
  expect_config '.layout.focused == 1' "the dragged pane took the focus" 3 '.layout.focused'
  if (($(region_diff "$before" "$after" 60 14 140 28) > 80)); then ok "headers swapped on screen"; else fail "headers unchanged (see $after)"; fi
}
