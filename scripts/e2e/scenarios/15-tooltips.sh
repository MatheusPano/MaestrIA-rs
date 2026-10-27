PLUS_X=352 PLUS_Y=90
GEAR_X=377 GEAR_Y=30

scenario() {
  local before hover open moved
  before=$(shot before)
  move "$PLUS_X" "$PLUS_Y"
  sleep 1.5
  hover=$(shot hover)
  if (($(region_diff "$before" "$hover" 366 92 60 26) > 150)); then ok "hovering + shows its tooltip"; else fail "no tooltip after hovering + (see $hover)"; fi
  move 900 500
  sleep 1

  click "$PLUS_X" "$PLUS_Y"
  sleep 1.5
  open=$(shot menu)
  move 300 "$PLUS_Y"
  sleep 0.8
  moved=$(shot menu-moved)
  if (($(region_diff "$before" "$open" 366 100 200 150) > 2000)); then ok "+ opened the menu"; else fail "+ did not open a menu (see $open)"; fi
  if (($(region_diff "$open" "$moved" 366 92 200 40) < 40)); then ok "no tooltip over the menu after clicking +"; else fail "a tooltip covered the menu after clicking + (see $open)"; fi
  key Escape
  move 900 500
  sleep 1

  key ctrl+comma
  sleep 1.5
  local keyed
  keyed=$(shot settings-key)
  if (($(region_diff "$before" "$keyed" 520 100 640 500) > 20000)); then ok "Ctrl+, opened settings"; else fail "Ctrl+, did not open settings (see $keyed)"; fi
  key Escape
  sleep 0.6
  if (($(region_diff "$before" "$(shot closed)" 520 100 640 500) < 2000)); then ok "Escape closed settings"; else fail "Escape did not close settings"; fi

  click "$GEAR_X" "$GEAR_Y"
  sleep 1.5
  open=$(shot settings)
  if (($(region_diff "$before" "$open" 520 100 640 500) > 20000)); then ok "gear opened settings"; else fail "settings did not open (see $open)"; fi
  if (($(region_diff "$keyed" "$open" 340 20 260 50) < 40)); then ok "no tooltip over the settings dialog after clicking the gear"; else fail "a tooltip stayed over settings after clicking the gear (see $open)"; fi
}
