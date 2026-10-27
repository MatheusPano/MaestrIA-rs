SEED='{"folders":[{"root":$root,"name":"proj","collapsed":false}],
  "layout":{"panes":[{"folderRoot":$root,"kind":"claude","cwd":$root}],"tree":0,"focused":0}}'

prepare() {
  mkdir -p "$HOME/.local/bin"
  cat >"$HOME/.local/bin/claude" <<'SH'
#!/bin/sh
sleep 2
cols=$(stty size </dev/tty | cut -d' ' -f2)
awk -v c="$cols" 'BEGIN { s = "Rivers"; while (length(s) + 6 <= c - 4) s = s " shape"; printf "\342\227\217 %s\r\n\033[2Cecosystems across the globe.\r\n", s }'
exec cat
SH
  chmod +x "$HOME/.local/bin/claude"
}

scenario() {
  sleep 3
  click 800 400
  shot reply >/dev/null
  key ctrl+shift+a
  key ctrl+shift+c
  local got
  got=$(clipboard)
  if grep -qxE 'Rivers( shape)+ ecosystems across the globe\.' <<<"$got"; then
    ok "copy in a claude tab joined the rows Claude wrapped"
  else
    fail "clipboard was not reflowed: $(printf '%s' "$got" | head -c 400 | tr '\n' '|')"
  fi
  if grep -q '●' <<<"$got"; then fail "the reply bullet was copied"; else ok "the reply bullet was left out"; fi
}
