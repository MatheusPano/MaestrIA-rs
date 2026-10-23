SEED='{"folders":[{"root":$root,"name":"proj","collapsed":false}],
  "layout":{"panes":[{"folderRoot":$root,"kind":"setup","cwd":$root,"setup":{"root":$root,"selected":"CLAUDE.md"}}],"tree":0,"focused":0}}'

prepare() { printf '# Projeto\n\nlinha um\n' >"$PROJECT/CLAUDE.md"; }

scenario() {
  local before typed saved
  before=$(shot before)
  click 1000 243
  key ctrl+End
  type_text "linha dois"
  typed=$(shot typed)
  if (($(region_diff "$before" "$typed" 660 190 600 120) > 200)); then ok "typed text shows in the editor"; else fail "editor unchanged after typing (see $typed)"; fi
  if grep -q "linha dois" "$PROJECT/CLAUDE.md"; then fail "file written before Ctrl+S"; else ok "nothing saved before Ctrl+S"; fi
  key ctrl+s
  sleep 0.8
  saved=$(shot saved)
  if [[ $(cat "$PROJECT/CLAUDE.md") == $'# Projeto\n\nlinha um\nlinha dois' ]]; then ok "Ctrl+S wrote the file"; else fail "CLAUDE.md after Ctrl+S: $(jq -Rs . <"$PROJECT/CLAUDE.md")"; fi
  if (($(region_diff "$typed" "$saved" 410 60 430 80) > 20)); then ok "the dirty marker cleared"; else fail "nothing changed on screen after saving (see $saved)"; fi
}
