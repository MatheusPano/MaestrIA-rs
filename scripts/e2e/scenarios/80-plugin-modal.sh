prepare() { install_probe_plugin; }

scenario() {
  local before modal clicked closed
  PLUGIN_DATA=$MAESTRIA_HOME/plugin-data/e2e.probe
  before=$(shot before)
  key ctrl+alt+m
  expect_json "$PLUGIN_DATA/commands.jsonl" 'index("modal") != null' "Ctrl+Alt+M ran the plugin command" 8
  sleep 1
  modal=$(shot modal)
  if (($(region_diff "$before" "$modal" 240 60 800 200) > 20000)); then ok "the plugin modal is on screen"; else fail "no modal (see $modal)"; fi

  click 900 160
  key End
  type_text " editada"
  click 322 212
  expect_json "$PLUGIN_DATA/actions.jsonl" 'any(.action == "clicou" and .viewId == "m" and .values.nota == "inicial editada")' "the button sent view.action with the typed field" 6
  sleep 1
  clicked=$(shot clicked)
  if (($(region_diff "$modal" "$clicked" 250 230 400 60) > 200)); then ok "view.update redrew the modal"; else fail "modal unchanged after view.update (see $clicked)"; fi

  key Escape
  closed=$(shot closed)
  if (($(region_diff "$before" "$closed" 240 60 800 200) < 2000)); then ok "Escape closed the modal"; else fail "modal still open after Escape (see $closed)"; fi
  key ctrl+alt+m
  expect_json "$PLUGIN_DATA/commands.jsonl" '[.[] | select(. == "modal")] | length == 2' "shortcuts still reach the app after the modal closed" 8
}
