SEED='{"folders":[{"root":$root,"name":"proj","collapsed":false}],
  "layout":{"panes":[{"folderRoot":$root,"kind":"shell","cwd":$root},{"folderRoot":$root,"kind":"shell","cwd":$root},{"folderRoot":$root,"kind":"shell","cwd":$root}],
            "tree":{"axis":"row","weights":[0.5,0.5],"children":[0,{"axis":"column","weights":[0.5,0.5],"children":[1,2]}]},"focused":0}}'

gutters() {
  python3 - "$@" <<'PY'
import sys
from PIL import Image
im = Image.open(sys.argv[1]).convert("RGB")
axis, at, lo = sys.argv[2], int(sys.argv[3]), int(sys.argv[4])
n = im.width if axis == "x" else im.height
px = (lambda i: im.getpixel((i, at))) if axis == "x" else (lambda i: im.getpixel((at, i)))
line = [px(i) for i in range(n)]
edges = [i for i in range(lo, n - 20) if sum(abs(a - b) for a, b in zip(line[i], line[i - 1])) > 12]
print(" ".join(str((a + b) // 2) for a, b in zip(edges, edges[1:]) if 6 <= b - a <= 16))
PY
}

burst_drag() {
  local axis=$1 at=$2 lo=$3 from=$4 dir=$5 steps=$6 i j s g ptr=$4 last=$4 back=0 off=0
  if [[ $axis == x ]]; then move "$from" "$at"; else move "$at" "$from"; fi
  on_display xdotool mousedown 1
  sleep 0.15
  for ((i = 1; i <= steps; i++)); do
    for ((j = 0; j < 10; j++)); do
      ptr=$((ptr + 3 * dir))
      if [[ $axis == x ]]; then on_display xdotool mousemove "$(abs_x "$ptr")" "$(abs_y "$at")"; else on_display xdotool mousemove "$(abs_x "$at")" "$(abs_y "$ptr")"; fi
    done
    s=$(shot "$axis-$i")
    g=$(gutters "$s" "$axis" "$at" "$lo")
    g=${g%% *}
    ((g * dir < last * dir)) && back=1
    ((g - ptr > 2 || ptr - g > 2)) && off=1
    log "step $i pointer=$ptr gutter=$g"
    last=$g
  done
  on_display xdotool mouseup 1
  nudge
  sleep "${E2E_SETTLE:-0.6}"
  ((back == 0)) && ok "$axis gutter never moved back during a burst drag" || fail "$axis gutter moved back during a burst drag"
  ((off == 0)) && ok "$axis gutter stayed under the pointer" || fail "$axis gutter drifted from the pointer"
}

scenario() {
  local s x y
  s=$(shot before)
  x=$(gutters "$s" x 300 440)
  x=${x%% *}
  [[ -n $x ]] || { fail "could not find the row gutter (see $s)"; return; }
  ok "row gutter at x=$x"
  burst_drag x 300 440 "$x" -1 4
  s=$(shot mid)
  y=$(gutters "$s" y $((WIN_W - 120)) 80)
  y=${y%% *}
  [[ -n $y ]] || { fail "could not find the column gutter (see $s)"; return; }
  ok "column gutter at y=$y"
  burst_drag y $((WIN_W - 120)) 80 "$y" -1 4
}
