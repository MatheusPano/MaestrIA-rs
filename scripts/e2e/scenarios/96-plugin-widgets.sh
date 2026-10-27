prepare() {
  mkdir -p "$MAESTRIA_HOME/plugins"
  cp -R "$HERE/fixtures/widgets-plugin" "$MAESTRIA_HOME/plugins/e2e.widgets"
}

scenario() {
  local data=$MAESTRIA_HOME/plugin-data/e2e.widgets before opened a b c moving listed grown closed
  before=$(shot before)
  key ctrl+alt+w
  expect_json "$data/commands.jsonl" 'index("widgets") != null' "Ctrl+Alt+W opened the widgets window" 8
  sleep 1.5
  opened=$(shot opened)
  if (($(region_diff "$before" "$opened" 240 60 800 500) > 20000)); then ok "the rfw modal is on screen"; else fail "no modal (see $opened)"; fi

  local x0=$(((WIN_W - 820) / 2 + 1)) y0=$(((WIN_H - 634) / 2 + 46))

  a=$(shot sprite-a); sleep 0.13; b=$(shot sprite-b); sleep 0.13; c=$(shot sprite-c)
  moving=$(($(region_diff "$a" "$b" $((x0 + 60)) "$y0" 40 40) + $(region_diff "$b" "$c" $((x0 + 60)) "$y0" 40 40)))
  if ((moving > 200)); then ok "the sprite animates by itself"; else fail "the sprite did not change frame (see $a $b $c)"; fi

  click $((x0 + 300)) $((y0 + 372))
  type_text "roda os testes"
  key Return
  expect_json "$data/actions.jsonl" 'any(.action == "send" and .values.text == "roda os testes" and .values.id == "chat")' "Enter sent the TextInput text" 6

  type_text "/co"
  listed=$(shot commands)
  if (($(region_diff "$opened" "$listed" "$x0" $((y0 + 350)) 700 60) > 1500)); then ok "the slash command list opened above the field"; else fail "no command list (see $listed)"; fi
  key Tab
  key Return
  expect_json "$data/actions.jsonl" 'any(.action == "send" and .values.text == "/compact")' "Tab completed /compact and Enter sent it" 6

  key Down
  expect_json "$data/actions.jsonl" 'any(.action == "arrow" and .values.key == "down")' "an arrow on the empty field went to the plugin" 6

  drag $((x0 + 4)) $((y0 + 70)) $((x0 + 400)) $((y0 + 205)) 16
  key ctrl+c
  local copied
  copied=$(clipboard)
  if [[ $copied == *$'primeiro parágrafo\nsegundo parágrafo'* && $copied == *"outro item"* ]]; then
    ok "the Selectable copied across markdown blocks and texts"
  else
    fail "selection copy: $(printf %q "$copied")"
  fi

  move $((x0 + 400)) $((y0 + 270))
  on_display xdotool click 4
  nudge
  expect_json "$data/actions.jsonl" 'any(.action == "zoom" and .values.scale > 1.05)' "the wheel zoomed the ZoomView and onZoom reported the scale" 6

  click $((x0 + 20)) $((y0 + 20))
  expect_json "$data/actions.jsonl" 'any(.action == "grow")' "the Anchor child was clicked" 6
  sleep 1
  grown=$(shot grown)
  if (($(region_diff "$opened" "$grown" 240 60 800 500) > 20000)); then ok "the second modal grew in"; else fail "no second modal (see $grown)"; fi

  key Escape
  expect_json "$data/events.jsonl" 'any(.type == "view.closed" and .viewId == "g")' "closing the modal sent view.closed" 6
  move $((WIN_W - 100)) $((WIN_H - 100))
  sleep 0.6
  closed=$(shot closed)
  if (($(region_diff "$before" "$closed" 240 60 800 500) < 3000)); then ok "the modal shrank away"; else fail "modal still on screen (see $closed)"; fi
}
