SEED='{"folders":[{"root":$root,"name":"proj","collapsed":false}],
  "layout":{"panes":[{"folderRoot":$root,"kind":"claude","cwd":$root}],"tree":0,"focused":0}}'

prepare() {
  mkdir -p "$HOME/.local/bin"
  cat >"$HOME/.local/bin/claude" <<SH
#!/bin/sh
while [ \$# -gt 0 ]; do
  [ "\$1" = --settings ] && printf '%s' "\$2" >"$E2E_TMP/settings.json"
  shift
done
exec cat
SH
  chmod +x "$HOME/.local/bin/claude"
}

statusline() {
  local cmd
  cmd=$(jq -r .statusLine.command "$E2E_TMP/settings.json")
  printf '%s' "$1" | sh -c "$cmd"
  sleep 1.5
}

scenario() {
  if ! wait_json "$E2E_TMP/settings.json" '.[0].statusLine.command | test("/status/")' 10; then
    fail "claude was not launched with a statusLine (settings: $(head -c 300 "$E2E_TMP/settings.json" 2>/dev/null))"
    return
  fi
  ok "claude got the maestria statusLine"
  local before shown cleared
  before=$(shot before)
  statusline '{"session_id":"s1","model":{"id":"claude-opus-4-1","display_name":"Opus"},"cost":{"total_cost_usd":0.42},"exceeds_200k_tokens":false,"context_window":{"total_input_tokens":15234,"total_output_tokens":4521,"context_window_size":200000,"used_percentage":90.4,"remaining_percentage":9.6,"current_usage":{"input_tokens":8500,"output_tokens":1200,"cache_creation_input_tokens":5000,"cache_read_input_tokens":166500}}}'
  shown=$(shot shown)
  local changed
  changed=$(region_diff "$before" "$shown" 0 0 "$WIN_W" 70)
  if ((changed > 150)); then ok "the context chip appeared in the pane header ($changed px)"; else fail "pane header unchanged after the status POST ($changed px, see $shown)"; fi
  local reds
  reds=$(convert "$shown" -crop "${WIN_W}x70+0+0" +repage -fx '(r>0.7 && g<0.5 && b<0.5) ? 1 : 0' -format '%[fx:int(mean*w*h)]' info: 2>/dev/null)
  if ((${reds:-0} > 20)); then ok "90% is drawn in red ($reds px)"; else fail "no red chip at 90% (${reds:-0} px, see $shown)"; fi
  statusline '{"context_window":{"context_window_size":200000,"used_percentage":null,"current_usage":null}}'
  cleared=$(shot cleared)
  changed=$(region_diff "$before" "$cleared" 0 0 "$WIN_W" 70)
  if ((changed < 150)); then ok "the chip left with the number after /compact"; else fail "chip still shown after a null usage ($changed px, see $cleared)"; fi
}
