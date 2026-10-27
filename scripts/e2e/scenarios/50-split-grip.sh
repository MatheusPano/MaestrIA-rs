SEED='{"folders":[{"root":$root,"name":"proj","collapsed":false}],
  "layout":{"panes":[{"folderRoot":$root,"kind":"shell","cwd":$root},{"folderRoot":$root,"kind":"shell","cwd":$root}],
            "tree":{"axis":"row","weights":[0.5,0.5],"children":[0,1]},"focused":0}}'

gap_x() {
  python3 - "$1" "$2" <<'PY'
import sys
from PIL import Image
im = Image.open(sys.argv[1]).convert("RGB")
y = int(sys.argv[2])
w = im.width
row = [im.getpixel((x, y)) for x in range(w)]
edges = [x for x in range(440, w - 20) if sum(abs(a - b) for a, b in zip(row[x], row[x - 1])) > 12]
mid = (440 + w) / 2
pairs = [(a, b) for a, b in zip(edges, edges[1:]) if 2 <= b - a <= 16]
pairs.sort(key=lambda p: abs((p[0] + p[1]) / 2 - mid))
print(int((pairs[0][0] + pairs[0][1]) / 2) if pairs else -1)
PY
}

scenario() {
  local before after x0 x1
  before=$(shot before)
  if wait_config '.layout.tree.children | length == 2' 1; then ok "seeded a two-pane row split"; else fail "split not restored: $(config | jq -c .layout)"; fi
  x0=$(gap_x "$before" 400)
  ((x0 > 0)) || { fail "could not find the split gutter (see $before)"; return; }
  ok "gutter at x=$x0"
  drag "$x0" 400 $((x0 - 200)) 400 16
  after=$(shot after)
  expect_config '.layout.tree.weights[0] < 0.42' "dragging the grip 200px left shrank the left weight" 5 '.layout.tree'
  x1=$(gap_x "$after" 400)
  if ((x1 > 0 && x1 < x0 - 120)); then ok "gutter moved to x=$x1"; else fail "gutter at x=$x1 after drag (was $x0, see $after)"; fi
}
