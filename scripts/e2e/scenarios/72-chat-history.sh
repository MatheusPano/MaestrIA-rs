CHAT_ID=11111111-2222-3333-4444-555555555555

prepare() {
  mkdir -p "$HOME/.local/bin"
  cat >"$HOME/.local/bin/claude" <<SH
#!/bin/sh
case " \$* " in *" --resume "*) ;; *) exit 0 ;; esac
printf '%s\n' "\$@" >"$E2E_TMP/claude.args"
exec cat
SH
  chmod +x "$HOME/.local/bin/claude"
}

scenario() {
  local before empty listed after
  before=$(shot before)
  click 118 673
  sleep 1
  empty=$(shot empty)
  if (($(region_diff "$before" "$empty" 250 300 780 120) > 5000)); then ok "history button opened Todas as conversas"; else fail "history dialog did not open (see $empty)"; fi
  key Escape
  if (($(region_diff "$before" "$(shot closed)" 250 300 780 120) < 2000)); then ok "Escape closed it"; else fail "Escape left the history dialog open"; fi

  local dir="$HOME/.claude/projects/$(printf '%s' "$PROJECT" | sed 's/[^A-Za-z0-9]/-/g')"
  mkdir -p "$dir"
  printf '%s\n' "{\"type\":\"user\",\"cwd\":\"$PROJECT\",\"message\":{\"content\":\"conversa de teste\"}}" >"$dir/$CHAT_ID.jsonl"
  click 118 673
  sleep 1
  listed=$(shot listed)
  if (($(region_diff "$empty" "$listed" 260 330 760 120) > 1000)); then ok "the seeded chat is listed"; else fail "no chat row (see $listed)"; fi

  click 342 340
  sleep 1.5
  after=$(shot resumed)
  expect_config ".layout.panes | any(.kind == \"claude\" and .sessionId == \"$CHAT_ID\")" "clicking the row resumed the chat in a claude pane" 6 '.layout'
  if grep -A1 -x -- --resume "$E2E_TMP/claude.args" 2>/dev/null | grep -qx -- "$CHAT_ID"; then ok "claude ran with --resume $CHAT_ID"; else fail "claude args: $(tr '\n' ' ' <"$E2E_TMP/claude.args" 2>/dev/null | cut -c1-300)"; fi
  if (($(region_diff "$listed" "$after" 250 330 780 60) > 2000)); then ok "dialog closed after resuming"; else fail "history dialog still open (see $after)"; fi
}
