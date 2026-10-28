prepare() {
  mkdir -p "$MAESTRIA_HOME/plugins"
  cp -R "$HERE/fixtures/zoom-plugin" "$MAESTRIA_HOME/plugins/e2e.zoom"
}

GREEN=40c040

color_box() {
  python3 - "$1" "$2" <<'PY'
import subprocess, sys
raw = subprocess.run(["convert", sys.argv[1], "ppm:-"], capture_output=True, check=True).stdout
head = raw.split(b"\n", 3)
w, h = map(int, head[1].split())
px = head[3]
want = bytes.fromhex(sys.argv[2])
xs, ys = [], []
for y in range(0, h, 2):
    row = y * w * 3
    for x in range(0, w, 2):
        if all(abs(px[row + 3 * x + i] - want[i]) <= 12 for i in range(3)):
            xs.append(x); ys.append(y)
print(f"{min(xs)} {min(ys)} {max(xs)} {max(ys)}" if xs else "none")
PY
}

run_of() {
  python3 - "$1" "$2" "$3" <<'PY'
import subprocess, sys
raw = subprocess.run(["convert", sys.argv[1], "ppm:-"], capture_output=True, check=True).stdout
head = raw.split(b"\n", 3)
w, _ = map(int, head[1].split())
px = head[3]
x0, y = int(sys.argv[2]), int(sys.argv[3])
at = lambda x: px[(y * w + x) * 3:(y * w + x) * 3 + 3]
c = at(x0)
same = lambda x: all(abs(at(x)[i] - c[i]) <= 12 for i in range(3))
l = x0
while l > 0 and same(l - 1): l -= 1
r = x0
while r < w - 1 and same(r + 1): r += 1
print(l, r)
PY
}

last_scale() { jq -s '[.[] | select(.action == "zoom") | .values.scale] | last // 1' "$1" 2>/dev/null || echo 1; }

wheel() {
  local button=$1 n=$2 i
  for ((i = 0; i < n; i++)); do on_display xdotool click "$button"; nudge; sleep 0.12; done
}

scenario() {
  local data=$MAESTRIA_HOME/plugin-data/e2e.zoom before zoomed back box gx0 gy0 gx1 gy1 vx0 vx1 px py l0 r0 l1 r1 k
  key ctrl+alt+z
  expect_json "$data/commands.jsonl" 'index("planta") != null' "Ctrl+Alt+Z opened the plan" 8
  sleep 1.5
  before=$(shot before)
  box=$(color_box "$before" "$GREEN")
  [[ $box != none ]] || { fail "the plan is not on screen (see $before)"; return; }
  read -r gx0 gy0 gx1 gy1 <<<"$box"
  px=$(((gx0 + gx1) / 2 - (gx1 - gx0) / 10)) py=$(((gy0 + gy1) / 2))
  read -r l0 r0 <<<"$(run_of "$before" "$px" "$py")"
  read -r vx0 _ _ _ <<<"$(color_box "$before" e04040)"
  read -r _ _ vx1 _ <<<"$(color_box "$before" e0c020)"
  ok "the fitted plan is on screen (green cell $l0..$r0 at row $py)"

  move "$px" "$py"
  wheel 4 4
  expect_json "$data/actions.jsonl" 'any(.action == "zoom" and .values.scale > 1.5)' "four notches zoomed in and onZoom reported it" 6
  zoomed=$(shot zoomed)
  k=$(last_scale "$data/actions.jsonl")
  if (($(hex_dist "$(pixel "$zoomed" "$px" "$py")" "$GREEN") <= 12)); then
    ok "the point under the pointer stayed on the same cell"
  else
    fail "the content left the pointer (pixel $(pixel "$zoomed" "$px" "$py"), see $zoomed)"
    return
  fi
  read -r l1 r1 <<<"$(run_of "$zoomed" "$px" "$py")"
  if python3 -c "import sys
k,p,l0,r0,l1,r1,v0,v1=map(float,sys.argv[1:])
el,er=max(p-k*(p-l0),v0),min(p+k*(r0-p),v1)
tol=4+0.03*k*(r0-l0)
sys.exit(0 if abs(l1-el)<=tol and abs(r1-er)<=tol else 1)" "$k" "$px" "$l0" "$r0" "$l1" "$r1" "$vx0" "$vx1"; then
    ok "the cell scaled ${k}x around the pointer horizontally ($l0..$r0 -> $l1..$r1)"
  else
    fail "the cell did not scale ${k}x around x=$px ($l0..$r0 -> $l1..$r1, see $zoomed)"
  fi

  wheel 5 14
  expect_json "$data/actions.jsonl" '[.[] | select(.action == "zoom")] | last | .values.scale < 1.01' "zooming out went back to the fitted plan" 6
  back=$(shot back)
  local w=$((gx1 - gx0)) h=$((gy1 - gy0))
  if (($(region_diff "$before" "$back" $((gx0 - w)) $((gy0 - h)) $((w * 4)) $((h * 4))) < 400)); then
    ok "the plan is back where it started"
  else
    fail "the plan did not come back (see $before $back)"
  fi
}
