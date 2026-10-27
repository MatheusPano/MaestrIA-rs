EDITOR_SRC=${E2E_EDITOR_PLUGIN:-$HOME/.maestria-rs/plugins/maestria.editor}

prepare() {
  [[ -f $EDITOR_SRC/maestria-plugin.json ]] || die "editor plugin not found at $EDITOR_SRC (set E2E_EDITOR_PLUGIN)"
  local node
  node=$(readlink -f "$(command -v node)") || die "node not found"
  mkdir -p "$MAESTRIA_HOME/plugins" "$MAESTRIA_HOME/plugin-data/maestria.editor" "$HOME/.local/bin"
  ln -s "$node" "$HOME/.local/bin/node"
  cp -R "$EDITOR_SRC" "$MAESTRIA_HOME/plugins/maestria.editor"
  mkdir -p "$PROJECT/src"
  printf 'const a = 1;\n// nota\nfunction soma(x) {\n  return x + a;\n}\n' >"$PROJECT/src/hello.js"
  printf '# Projeto\n\ntexto\n' >"$PROJECT/README.md"
  git -C "$PROJECT" add -A && git -C "$PROJECT" -c user.email=e2e@x -c user.name=e2e commit -qm init
  jq -n --arg root "$PROJECT" --arg f "$PROJECT/src/hello.js" \
    '{root:$root, recent:[$root], expanded:[($root+"/src")], tabs:[$f], active:0}' \
    >"$MAESTRIA_HOME/plugin-data/maestria.editor/state.json"
}

scenario() {
  local before opened typed saved found
  before=$(shot before)
  click 26 65
  sleep 3
  opened=$(shot opened)
  if (($(region_diff "$before" "$opened" 420 60 840 500) > 20000)); then ok "the rail icon opened the editor screen"; else fail "no editor screen (see $opened)"; fi
  if (($(region_colors "$opened" 470 120 500 100) > 6)); then ok "the file shows with syntax colours"; else fail "the editor area is flat (see $opened)"; fi

  key ctrl+End
  type_text "// e2e"
  sleep 1.5
  typed=$(shot typed)
  if grep -q "e2e" "$PROJECT/src/hello.js"; then fail "file written before Ctrl+S"; else ok "nothing saved before Ctrl+S"; fi
  if jq -e '.["'"$PROJECT/src/hello.js"'"] | test("e2e")' "$MAESTRIA_HOME/plugin-data/maestria.editor/unsaved.json" >/dev/null 2>&1 || sleep 2 && jq -e '.["'"$PROJECT/src/hello.js"'"] | test("e2e")' "$MAESTRIA_HOME/plugin-data/maestria.editor/unsaved.json" >/dev/null 2>&1; then
    ok "the plugin got the typed text (hot-exit backup)"
  else
    fail "the plugin never got a change (see $typed)"
  fi

  key ctrl+s
  sleep 1.5
  saved=$(shot saved)
  if tail -n1 "$PROJECT/src/hello.js" | grep -q "^// e2e$"; then ok "Ctrl+S wrote the file"; else fail "hello.js after Ctrl+S: $(jq -Rs . <"$PROJECT/src/hello.js")"; fi

  key ctrl+f
  type_text "soma"
  key Return
  found=$(shot found)
  if (($(region_diff "$saved" "$found" 420 60 840 400) > 500)); then ok "Ctrl+F opened the find bar"; else fail "no find bar (see $found)"; fi
  key Escape

}
