prepare() {
  mkdir -p "$MAESTRIA_HOME/plugins"
  cp -R "$HERE/fixtures/argo-plugin" "$MAESTRIA_HOME/plugins/e2e.argo"
  cp "$REPO/crates/mx-rfw/tests/fixtures/maestria-plugins__argocd__argo.rfwtxt" "$MAESTRIA_HOME/plugins/e2e.argo/argo.rfwtxt"
}

card() {
  python3 - "$1" <<'PY'
import sys
from PIL import Image
im = Image.open(sys.argv[1]).convert("RGB")
W, H = im.size
bg = im.getpixel((W - 30, H - 60))
yellow = lambda p: p[0] > 150 and p[1] > 110 and p[2] < 130 and p[0] - p[2] > 70
best = (0, 0, 0, 0)
for x in range(W):
    run = top = 0
    for y in range(H):
        if yellow(im.getpixel((x, y))):
            run += 1
            if run > best[0]:
                best = (run, x, y - run + 1, y)
        else:
            run = 0
run, _, top, bottom = best
if run < 120:
    print("none")
    sys.exit()
cols = [x for x in range(W) if sum(yellow(im.getpixel((x, y))) for y in range(top, bottom, 4)) > (bottom - top) / 6]
left, right = cols[0], cols[-1]
far = lambda p: max(abs(a - b) for a, b in zip(p, bg)) > 40
pills = range(top + 34, top + 66)
spill = sum(far(im.getpixel((x, y))) for x in range(right + 3, min(right + 12, W)) for y in range(top + 4, min(bottom - 4, top + 160)) if y not in pills)
values = sum(far(im.getpixel((x, y))) for x in range(left + 92, right - 12) for y in range(top + 70, top + 150))
print(left, top, right, bottom, spill, values)
PY
}

check() {
  local img=$1 what=$2 left top right bottom spill values
  read -r left top right bottom spill values <<<"$(card "$img")"
  if [[ $left == none || -z $left ]]; then fail "$what: no long-name card found (see $img)"; return; fi
  if ((spill == 0)); then ok "$what: card $left..$right keeps its text inside"; else fail "$what: $spill text pixels right of the card at x=$right (see $img)"; fi
  if ((values > 150)); then ok "$what: the row values are drawn"; else fail "$what: row values missing ($values px, see $img)"; fi
  if ((right - left > 150 && right < WIN_W - 8)); then ok "$what: the whole card fits the window"; else fail "$what: card cut by the window edge ($left..$right, see $img)"; fi
}

resize() {
  on_display xdotool windowsize --sync "$WIN_ID" "$1" "$2" 2>/dev/null
  WIN_W=$1 WIN_H=$2
  nudge
  sleep 1
}

scenario() {
  key ctrl+alt+g
  sleep 2
  check "$(shot wide)" "1280x720"
  resize 900 650
  check "$(shot medium)" "900x650"
  resize 640 600
  check "$(shot narrow)" "640x600"
  resize 1280 720
  move 408 400
  on_display xdotool mousedown 1
  local x step
  for ((step = 1; step <= 10; step++)); do
    on_display xdotool mousemove "$(abs_x $((408 + step * 40)))" "$(abs_y 400)"
    sleep 0.05
    if ((step == 5)); then check "$(shot dragging)" "while dragging the sidebar edge"; fi
  done
  on_display xdotool mouseup 1
  nudge
  sleep 0.8
  check "$(shot dragged)" "after dragging the sidebar edge"
}
