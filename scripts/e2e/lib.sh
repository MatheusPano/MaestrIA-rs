# shellcheck shell=bash

E2E_FAILS=0

die() { echo "e2e: $*" >&2; exit 2; }

on_display() {
  [[ -n ${E2E_DISPLAY:-} && $E2E_DISPLAY != ":0" && $E2E_DISPLAY != ":0.0" ]] || die "refusing to touch display '${E2E_DISPLAY:-}'"
  DISPLAY=$E2E_DISPLAY "$@"
}

log() { echo "  [$E2E_SCENARIO] $*"; }

ok() { log "ok   $*"; }

fail() {
  E2E_FAILS=$((E2E_FAILS + 1))
  log "FAIL $*"
}

abs_x() { echo $((WIN_X + $1)); }
abs_y() { echo $((WIN_Y + $1)); }

move() { on_display xdotool mousemove --sync "$(abs_x "$1")" "$(abs_y "$2")"; sleep 0.15; nudge; }

click() { move "$1" "$2"; on_display xdotool click "${3:-1}"; nudge; sleep "${E2E_SETTLE:-0.6}"; }

rclick() { click "$1" "$2" 3; }

tclick() { move "$1" "$2"; on_display xdotool click --repeat 3 --delay 60 1; nudge; sleep "${E2E_SETTLE:-0.6}"; }

drag() {
  local x1=$1 y1=$2 x2=$3 y2=$4 steps=${5:-12} i
  move "$x1" "$y1"
  on_display xdotool mousedown 1
  sleep 0.15
  for ((i = 1; i <= steps; i++)); do
    on_display xdotool mousemove "$(abs_x $((x1 + (x2 - x1) * i / steps)))" "$(abs_y $((y1 + (y2 - y1) * i / steps)))"
    sleep 0.04
  done
  sleep 0.15
  on_display xdotool mouseup 1
  nudge
  sleep "${E2E_SETTLE:-0.6}"
}

type_text() { on_display xdotool type --delay 40 -- "$1"; nudge; sleep 0.3; }

key() { on_display xdotool key --delay 60 "$@"; nudge; sleep "${E2E_SETTLE:-0.6}"; }

# gpui's X11 loop only wakes on fd readiness; events xcb already queued while waiting for a reply stall until the next one arrives.
nudge() { on_display xprop -id "$WIN_ID" -f _E2E_NUDGE 32c -set _E2E_NUDGE "$RANDOM" 2>/dev/null; }

shot() {
  local file="$E2E_SHOTS/$E2E_SCENARIO-$1.png"
  nudge
  sleep 0.4
  on_display import -window "$WIN_ID" "$file" 2>/dev/null || on_display import -window root "$file"
  echo "$file"
}

pixel() { convert "$1" -format "%[hex:u.p{$2,$3}]" info: | cut -c1-6 | tr 'A-F' 'a-f'; }

region_colors() { convert "$1" -crop "$4x$5+$2+$3" +repage -unique-colors -format '%w' info:; }

region_diff() {
  compare -metric AE -crop "$5x$6+$3+$4" "$1" "$2" null: 2>&1 | awk '{print int($1)}'
}

hex_dist() {
  local a=$1 b=$2
  python3 -c "import sys
a,b=sys.argv[1],sys.argv[2]
print(max(abs(int(a[i:i+2],16)-int(b[i:i+2],16)) for i in (0,2,4)))" "$a" "$b"
}

config() { cat "$MAESTRIA_HOME/config.json" 2>/dev/null; }

wait_config() {
  local filter=$1 deadline=$((SECONDS + ${2:-5}))
  while ((SECONDS < deadline)); do
    if config | jq -e "$filter" >/dev/null 2>&1; then return 0; fi
    sleep 0.25
  done
  return 1
}

expect_config() {
  if wait_config "$1" "${3:-5}"; then ok "$2"; else fail "$2 (config.json: $(config | jq -c "${4:-.}" 2>/dev/null))"; fi
}

clipboard() { on_display xclip -o -selection clipboard 2>/dev/null; }
