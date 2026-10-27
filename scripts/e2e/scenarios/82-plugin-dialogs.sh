prepare() { install_probe_plugin; }

scenario() {
  local before pick filtered input form after
  PLUGIN_DATA=$MAESTRIA_HOME/plugin-data/e2e.probe
  ANSWERS=$PLUGIN_DATA/answers.jsonl
  before=$(shot before)
  key ctrl+alt+j
  expect_json "$PLUGIN_DATA/commands.jsonl" 'index("ask") != null' "Ctrl+Alt+J ran the plugin command" 8
  sleep 1
  pick=$(shot pick)
  if (($(region_diff "$before" "$pick" 390 150 500 200) > 10000)); then ok "quick pick opened"; else fail "no quick pick (see $pick)"; fi
  type_text "be"
  filtered=$(shot filtered)
  if (($(region_diff "$pick" "$filtered" 390 280 500 80) > 1000)); then ok "typing filtered the list"; else fail "list unchanged after typing (see $filtered)"; fi
  key Return
  expect_json "$ANSWERS" 'any(.pick == "beta")' "Enter picked beta" 5

  sleep 0.5
  input=$(shot input)
  if (($(region_diff "$before" "$input" 390 150 500 100) > 5000)); then ok "quick input opened"; else fail "no quick input (see $input)"; fi
  type_text "novo texto"
  key Return
  expect_json "$ANSWERS" 'any(.input == "novo texto")' "typing replaced the preselected value" 5

  sleep 0.5
  form=$(shot form)
  if (($(region_diff "$before" "$form" 700 300 460 220) > 10000)); then ok "form opened"; else fail "no form (see $form)"; fi
  key Return
  sleep 0.5
  if wait_json "$ANSWERS" 'any(has("form"))' 1; then fail "form submitted with the required field empty"; else ok "required field held the form open"; fi
  type_text "fulano"
  key Return
  expect_json "$ANSWERS" 'any(.form == {"action":"salvar","values":{"nome":"fulano","tipo":"dois"}})' "form answered salvar with both values" 5
  after=$(shot after)
  if (($(region_diff "$before" "$after" 390 150 800 400) < 2000)); then ok "no dialog left on screen"; else fail "a dialog is still drawn (see $after)"; fi
}
