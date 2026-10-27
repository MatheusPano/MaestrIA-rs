SEED='{"folders":[{"root":$root,"name":"proj","collapsed":false}],
  "layout":{"panes":[{"folderRoot":$root,"kind":"reader","cwd":$root,"doc":{"source":"file","path":($root+"/a.md")}}],"tree":0,"focused":0}}'

prepare() {
  printf '# Documento A\n\nVeja [o documento B](b.md) e [um que falta](nada.md).\n' >"$PROJECT/a.md"
  printf '# Documento B\n\nChegou no B.\n' >"$PROJECT/b.md"
}

scenario() {
  local before missing followed
  before=$(shot before)
  if (($(region_colors "$before" 460 60 400 60) > 8)); then ok "reader rendered a.md"; else fail "reader looks empty (see $before)"; fi
  local link_y=131 b_x missing_x
  read -r b_x missing_x < <(python3 - "$before" "$link_y" <<'PY'
import sys
from PIL import Image
im = Image.open(sys.argv[1]).convert("RGB")
y = int(sys.argv[2])
xs = [x for x in range(420, im.width) if (lambda p: p[2] > 150 and p[2] > p[0] + 40)(im.getpixel((x, y)))]
runs, start = [], None
for a, b in zip(xs, xs[1:] + [None]):
    start = a if start is None else start
    if b is None or b - a > 12:
        runs.append((start + a) // 2); start = None
print(*(runs[:2] if len(runs) >= 2 else [-1, -1]))
PY
)
  ((b_x > 0)) || { fail "could not find the two links on row y=$link_y (see $before)"; return; }
  ok "links at x=$b_x and x=$missing_x"

  click "$missing_x" "$link_y"
  sleep 0.5
  missing=$(shot missing)
  expect_config '.layout.panes | length == 1' "a broken link opened nothing" 2 '.layout'
  if (($(region_diff "$before" "$missing" 420 560 860 160) > 500)); then ok "a broken link showed a banner"; else fail "no banner for the broken link (see $missing)"; fi

  click "$b_x" "$link_y"
  sleep 1
  followed=$(shot followed)
  expect_config '.layout.panes | any(.kind == "reader" and (.doc.path | endswith("/b.md")))' "clicking the link opened b.md in a reader" 5 '.layout'
  if (($(region_diff "$before" "$followed" 420 60 860 120) > 300)); then ok "b.md is on screen"; else fail "screen unchanged after following (see $followed)"; fi
}
