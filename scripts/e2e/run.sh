#!/usr/bin/env bash
set -uo pipefail

HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
REPO=$(cd "$HERE/../.." && pwd)
BIN=${MAESTRIA_BIN:-$REPO/target-e2e/debug/maestria}
SCREEN=${E2E_SCREEN:-1280x720}
LVP_ICD=/usr/share/vulkan/icd.d/lvp_icd.json

# shellcheck source=lib.sh
source "$HERE/lib.sh"

[[ -x $BIN ]] || die "binary not found: $BIN (build with CARGO_TARGET_DIR=target-e2e cargo build -p mx-app)"
[[ -f $LVP_ICD ]] || die "mesa lavapipe ICD missing ($LVP_ICD): apt install mesa-vulkan-drivers"
for tool in Xvfb xdotool import convert compare jq xclip dbus-run-session; do
  command -v "$tool" >/dev/null || die "missing tool: $tool"
done

E2E_OUT=${E2E_OUT:-$(mktemp -d "${TMPDIR:-/tmp}/maestria-e2e.XXXXXX")}
E2E_SHOTS=$E2E_OUT/shots
mkdir -p "$E2E_SHOTS"
XVFB_PID=
APP_PID=

stop_app() {
  [[ -n $APP_PID ]] || return 0
  pkill -TERM -P "$APP_PID" 2>/dev/null
  kill "$APP_PID" 2>/dev/null
  for _ in $(seq 15); do kill -0 "$APP_PID" 2>/dev/null || break; sleep 0.2; done
  pkill -KILL -P "$APP_PID" 2>/dev/null
  kill -9 "$APP_PID" 2>/dev/null
  wait "$APP_PID" 2>/dev/null
  APP_PID=
}

teardown() {
  stop_app
  [[ -n $XVFB_PID ]] && kill "$XVFB_PID" 2>/dev/null
  [[ -n ${E2E_TMP:-} && -d $E2E_TMP ]] && rm -rf "$E2E_TMP"
}
trap teardown EXIT
trap 'exit 130' INT TERM

start_display() {
  local fd_file=$E2E_OUT/display
  nice -n 10 Xvfb -displayfd 3 -screen 0 "${SCREEN}x24" -nolisten tcp -noreset 3>"$fd_file" 2>"$E2E_OUT/xvfb.log" &
  XVFB_PID=$!
  for _ in $(seq 50); do [[ -s $fd_file ]] && break; sleep 0.1; done
  [[ -s $fd_file ]] || die "Xvfb did not start (see $E2E_OUT/xvfb.log)"
  E2E_DISPLAY=:$(tr -d '\n' <"$fd_file")
  export E2E_DISPLAY
}

seed_home() {
  E2E_TMP=$(mktemp -d "${TMPDIR:-/tmp}/maestria-e2e-home.XXXXXX")
  export HOME=$E2E_TMP/home MAESTRIA_HOME=$E2E_TMP/home/.maestria
  PROJECT=$E2E_TMP/proj
  mkdir -p "$MAESTRIA_HOME" "$HOME/.claude/projects" "$HOME/.config" "$PROJECT" "$E2E_TMP/run"
  chmod 700 "$E2E_TMP/run"
  git -C "$PROJECT" init -q 2>/dev/null
  printf 'PS1="$ "\nunset PROMPT_COMMAND\n' >"$HOME/.bashrc"
  printf '[ -f ~/.bashrc ] && . ~/.bashrc\n' >"$HOME/.bash_profile"
  touch "$HOME/.sudo_as_admin_successful" "$HOME/.hushlogin"
  local seed=${SEED:-'{"folders":[{"root":$root,"name":"proj","collapsed":false}]}'}
  jq -n --arg root "$PROJECT" "$seed" >"$MAESTRIA_HOME/config.json"
}

start_app() {
  local log=$E2E_OUT/$E2E_SCENARIO.app.log
  (
    cd "$PROJECT" || exit 1
    exec env -i \
      PATH=/usr/local/bin:/usr/bin:/bin HOME="$HOME" MAESTRIA_HOME="$MAESTRIA_HOME" USER="${USER:-e2e}" \
      DISPLAY="$E2E_DISPLAY" XDG_RUNTIME_DIR="$E2E_TMP/run" XDG_CONFIG_HOME="$HOME/.config" \
      SHELL=/bin/bash LANG=C.UTF-8 RUST_LOG="${RUST_LOG:-info}" RUST_BACKTRACE=1 \
      VK_DRIVER_FILES="$LVP_ICD" VK_ICD_FILENAMES="$LVP_ICD" LIBGL_ALWAYS_SOFTWARE=1 \
      MAESTRIA_REDUCE_MOTION=1 \
      nice -n 10 dbus-run-session -- "$BIN"
  ) >"$log" 2>&1 &
  APP_PID=$!
  WIN_ID=$(on_display timeout 30 xdotool search --sync --onlyvisible --name MaestrIA 2>/dev/null | head -1)
  [[ -n $WIN_ID ]] || { fail "window never appeared (log: $log)"; return 1; }
  on_display xdotool windowmove --sync "$WIN_ID" 0 0 2>/dev/null
  eval "$(on_display xdotool getwindowgeometry --shell "$WIN_ID")"
  WIN_X=$X WIN_Y=$Y WIN_W=$WIDTH WIN_H=$HEIGHT
  export WIN_ID WIN_X WIN_Y WIN_W WIN_H
  local first
  for _ in $(seq 60); do
    first=$(shot boot)
    [[ $(region_colors "$first" 0 0 "$WIN_W" "$WIN_H") -gt 8 ]] && break
    sleep 0.5
  done
  sleep "${E2E_BOOT_SETTLE:-1.5}"
  on_display xdotool mousemove "$((WIN_X + WIN_W - 2))" "$((WIN_Y + WIN_H - 2))"
  if grep -q "Failed to create surface\|PlatformNotSupported" "$log"; then
    fail "renderer did not start (log: $log)"
    return 1
  fi
}

run_scenario() {
  local file=$1
  E2E_SCENARIO=$(basename "$file" .sh)
  export E2E_SCENARIO
  echo "== $E2E_SCENARIO"
  (
    E2E_FAILS=0
    SEED=
    trap 'stop_app; [[ -n ${E2E_TMP:-} ]] && rm -rf "$E2E_TMP"' EXIT
    unset -f scenario prepare
    # shellcheck disable=SC1090
    source "$file"
    seed_home
    if declare -F prepare >/dev/null; then prepare; fi
    if start_app; then
      scenario
      kill -0 "$APP_PID" 2>/dev/null || fail "app exited during the scenario (log: $E2E_OUT/$E2E_SCENARIO.app.log)"
      shot end >/dev/null
    fi
    cp "$MAESTRIA_HOME/config.json" "$E2E_OUT/$E2E_SCENARIO.config.json" 2>/dev/null
    stop_app
    rm -rf "$E2E_TMP"
    exit $((E2E_FAILS > 0))
  )
}

start_display
echo "e2e: display $E2E_DISPLAY (Xvfb $SCREEN, lavapipe), artifacts in $E2E_OUT"

if (($# > 0)); then
  files=()
  for name in "$@"; do if [[ -f $name ]]; then files+=("$name"); else files+=("$HERE/scenarios/$name.sh"); fi; done
else
  files=("$HERE"/scenarios/*.sh)
fi

passed=() failed=()
for file in "${files[@]}"; do
  [[ -f $file ]] || die "no such scenario: $file"
  if run_scenario "$file"; then passed+=("$(basename "$file" .sh)"); else failed+=("$(basename "$file" .sh)"); fi
done

echo
echo "passed: ${#passed[@]}  ${passed[*]}"
echo "failed: ${#failed[@]}  ${failed[*]}"
echo "artifacts: $E2E_OUT"
((${#failed[@]} == 0))
