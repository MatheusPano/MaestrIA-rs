SEED='{"folders":[{"root":$root,"name":"proj","collapsed":false}],
  "layout":{"panes":[{"folderRoot":$root,"kind":"claude","cwd":$root}],"tree":0,"focused":0}}'

prepare() {
  mkdir -p "$HOME/.local/bin" "$PROJECT/docs"
  printf '# Plano\n\nPasso um.\n' >"$PROJECT/docs/plano.md"
  printf '# Resumo\n\nTudo certo.\n' >"$PROJECT/docs/resumo.md"
  cat >"$HOME/.local/bin/claude" <<'SH'
#!/bin/sh
sleep 1
printf '\033[H\033[2J'
printf '\033]8;;file://%s/docs/resumo.md\033\\resumo-marcado-pelo-claude\033]8;;\033\\ pronto\r\n' "$PWD"
printf 'docs/plano.md:3 foi escrito.\r\n'
printf '\033[?1000h\033[?1006h'
stty raw -echo
exec cat >/dev/null
SH
  chmod +x "$HOME/.local/bin/claude"
}

scenario() {
  sleep 3
  local before
  before=$(shot before)
  read -r ROW0_Y ROW1_Y TEXT_X < <(python3 - "$before" <<'PY'
import sys
from PIL import Image
im = Image.open(sys.argv[1]).convert("RGB")
bg = im.getpixel((im.width - 20, im.height - 60))
def ink(p):
    return max(abs(p[i] - bg[i]) for i in range(3)) > 60
rows, inrow = [], False
for y in range(50, 300):
    hit = any(ink(im.getpixel((x, y))) for x in range(440, 700))
    if hit and not inrow:
        start = y
    if not hit and inrow:
        rows.append((start + y) // 2)
    inrow = hit
xs = [x for x in range(420, 900) if rows and ink(im.getpixel((x, rows[0])))]
print(rows[0] if rows else -1, rows[1] if len(rows) > 1 else -1, (xs[0] + 12) if xs else -1)
PY
)
  ((ROW1_Y > 0 && TEXT_X > 0)) || { fail "could not find the two printed rows (see $before)"; return; }
  ok "rows at y=$ROW0_Y,$ROW1_Y text from x=$TEXT_X"

  click "$TEXT_X" "$ROW1_Y"
  if wait_config '.layout.panes | any(.kind == "reader" and (.doc.path | endswith("/docs/plano.md")))' 5; then
    ok "clicking docs/plano.md:3 opened it in the reader"
  else
    fail "no reader for docs/plano.md (config.json: $(config | jq -c .layout))"
  fi

  click "$TEXT_X" "$ROW0_Y"
  if wait_config '.layout.panes | any(.kind == "reader" and (.doc.path | endswith("/docs/resumo.md")))' 5; then
    ok "clicking an OSC 8 file:// link opened it in the reader"
  else
    fail "no reader for docs/resumo.md (config.json: $(config | jq -c .layout))"
  fi
  shot after >/dev/null
}
