prepare() {
  cp -R "$HERE/fixtures/probe-plugin" "$E2E_TMP/sonda"
}

staged_count() { find "$MAESTRIA_HOME/plugins/.staging" -mindepth 1 -maxdepth 1 2>/dev/null | wc -l; }

wait_staged() {
  local deadline=$((SECONDS + 10))
  while ((SECONDS < deadline)) && (($(staged_count) == 0)); do sleep 0.25; done
  sleep 1
}

from_url() {
  click 15 65
  sleep 1
  click 764 615
  sleep 1
}

ask_and_stage() {
  from_url
  type_text "$E2E_TMP/sonda"
  key Return
  wait_staged
}

scenario() {
  local before catalog ask trust back after
  before=$(shot before)
  click 15 65
  catalog=$(shot catalog)
  if (($(region_diff "$before" "$catalog" 340 80 600 560) > 5000)); then ok "rail + opened the catalog"; else fail "no catalog (see $catalog)"; fi
  click 764 615
  ask=$(shot ask)
  if (($(region_diff "$catalog" "$ask" 340 80 600 560) > 5000)); then ok "De uma URL opened Instalar plugin"; else fail "no install dialog (see $ask)"; fi
  type_text "$E2E_TMP/sonda"
  key Return
  wait_staged
  trust=$(shot trust)
  if (($(region_diff "$ask" "$trust" 390 360 500 120) > 5000)); then ok "Enter showed the trust screen"; else fail "no trust screen (see $trust)"; fi
  if (($(staged_count) == 1)) && [[ ! -e $MAESTRIA_HOME/plugins/e2e.probe ]]; then ok "staged, not installed yet"; else fail "staging=$(staged_count), installed=$([[ -e $MAESTRIA_HOME/plugins/e2e.probe ]] && echo yes || echo no)"; fi

  click 740 438
  back=$(shot back)
  if (($(region_diff "$trust" "$back" 390 360 500 120) > 5000)); then ok "Voltar went back to the source field"; else fail "Voltar did nothing (see $back)"; fi
  if (($(staged_count) == 0)); then ok "Voltar discarded the staging"; else fail "staging left after Voltar: $(staged_count)"; fi

  key Return
  wait_staged
  key Escape
  after=$(shot escaped)
  if (($(region_diff "$before" "$after" 390 280 780 330) < 2000)); then ok "Escape on the trust screen closed the dialog"; else fail "dialog still open after Escape (see $after)"; fi
  if (($(staged_count) == 0)) && [[ ! -e $MAESTRIA_HOME/plugins/e2e.probe ]]; then ok "Escape discarded without installing"; else fail "after Escape: staging=$(staged_count)"; fi

  ask_and_stage
  click 830 438
  local deadline=$((SECONDS + 10))
  while ((SECONDS < deadline)) && [[ ! -f $MAESTRIA_HOME/plugins/e2e.probe/main.py ]]; do sleep 0.25; done
  sleep 1
  after=$(shot installed)
  if [[ -f $MAESTRIA_HOME/plugins/e2e.probe/maestria-plugin.json && -f $MAESTRIA_HOME/plugins/e2e.probe/main.py ]]; then ok "Confio, instalar copied the plugin"; else fail "plugin dir missing after install"; fi
  if (($(staged_count) == 0)); then ok "no staging left"; else fail "staging left: $(staged_count)"; fi
  if (($(region_diff "$before" "$after" 390 280 780 330) < 3000)); then ok "dialog closed after install"; else fail "dialog still open after install (see $after)"; fi
  key ctrl+alt+m
  expect_json "$MAESTRIA_HOME/plugin-data/e2e.probe/commands.jsonl" 'index("modal") != null' "the installed plugin answers its shortcut" 8
}
