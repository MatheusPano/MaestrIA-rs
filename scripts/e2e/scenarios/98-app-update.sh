# Needs MAESTRIA_BIN built with MAESTRIA_VERSION=3.0.0.

free_port() { python3 -I -c 'import socket; s = socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1])'; }

prepare() {
  local srv=$E2E_TMP/srv deb=maestria-rs_3.1.0-1_amd64.deb port
  port=$(free_port)
  UPDATE_BASE=http://127.0.0.1:$port
  mkdir -p "$srv"
  printf 'pacote falso 3.1.0\n' >"$srv/$deb"
  printf 'flutter\n' >"$srv/maestria_3.1.0-1_amd64.deb"
  (cd "$srv" && sha256sum "$deb" maestria_3.1.0-1_amd64.deb >SHA256SUMS)
  jq -n --arg b "$UPDATE_BASE" --arg deb "$deb" '{tag_name: "v3.1.0", assets: [
      {name: "maestria_3.1.0-1_amd64.deb", browser_download_url: ($b + "/maestria_3.1.0-1_amd64.deb")},
      {name: $deb, browser_download_url: ($b + "/" + $deb)},
      {name: "SHA256SUMS", browser_download_url: ($b + "/SHA256SUMS")}]}' >"$srv/latest.json"
  timeout 180 python3 -I -m http.server "$port" --bind 127.0.0.1 --directory "$srv" >"$E2E_OUT/$E2E_SCENARIO.http.log" 2>&1 &
  for _ in $(seq 40); do curl -sf "$UPDATE_BASE/latest.json" >/dev/null && break; sleep 0.1; done

  printf '#!/bin/sh\nprintf "%%s\\n" "$@" >"%s/install-args"\ncp "$1" "%s/installed.deb"\n' "$E2E_TMP" "$E2E_TMP" >"$E2E_TMP/fake-install"
  printf '#!/bin/sh\ntouch "%s/relaunched"\n' "$E2E_TMP" >"$E2E_TMP/fake-relaunch"
  chmod +x "$E2E_TMP/fake-install" "$E2E_TMP/fake-relaunch"
  E2E_APP_ENV=(
    "MAESTRIA_UPDATE_API=$UPDATE_BASE/latest.json"
    MAESTRIA_UPDATE_ENABLED=1
    "MAESTRIA_UPDATE_INSTALL=$E2E_TMP/fake-install"
    "MAESTRIA_UPDATE_RELAUNCH=$E2E_TMP/fake-relaunch"
  )
  E2E_EXPECT_EXIT=1
}

wait_file() {
  local deadline=$((SECONDS + ${2:-10}))
  while ((SECONDS < deadline)); do [[ -e $1 ]] && return 0; sleep 0.25; done
  return 1
}

scenario() {
  local before settings section toggled chip dialog staged=$MAESTRIA_HOME/update/maestria-rs_3.1.0-1_amd64.deb
  before=$(shot before)
  click 377 30
  settings=$(shot settings)
  click 300 "${E2E_UPDATES_ROW:-308}"
  section=$(shot section)
  if (($(region_diff "$settings" "$section" 420 100 600 300) > 3000)); then ok "Atualizações section opened"; else fail "Atualizações section did not open (see $section)"; fi

  click "${E2E_SWITCH_X:-1000}" "${E2E_SWITCH_Y:-145}"
  expect_config '.autoUpdate == true' "autoUpdate saved as true" 5 '.autoUpdate'
  toggled=$(shot toggled)
  if wait_file "$staged" 15; then ok "the verified .deb was staged in update/"; else fail "nothing staged (log: $E2E_OUT/$E2E_SCENARIO.app.log)"; fi
  sleep 1
  key Escape
  chip=$(shot chip)
  if (($(region_diff "$before" "$chip" 1080 690 150 30) > 50)); then ok "the update chip appeared next to the bell"; else fail "no update chip (see $chip)"; fi

  click "${E2E_CHIP_X:-1192}" "${E2E_CHIP_Y:-705}"
  dialog=$(shot dialog)
  if (($(region_diff "$chip" "$dialog" 340 200 600 300) > 5000)); then ok "the chip opened Atualizar pra 3.1.0?"; else fail "no confirm dialog (see $dialog)"; fi
  click "${E2E_NOW_X:-830}" "${E2E_NOW_Y:-420}"

  if wait_file "$E2E_TMP/install-args" 10; then ok "the install command ran"; else fail "install command never ran"; fi
  if [[ $(cat "$E2E_TMP/install-args" 2>/dev/null) == "$staged" ]]; then ok "it got the staged .deb path"; else fail "install args: $(cat "$E2E_TMP/install-args" 2>/dev/null)"; fi
  if cmp -s "$E2E_TMP/installed.deb" "$E2E_TMP/srv/maestria-rs_3.1.0-1_amd64.deb"; then ok "the installed file is the verified maestria-rs package"; else fail "installed file differs from the served package"; fi
  local deadline=$((SECONDS + 15))
  while ((SECONDS < deadline)) && kill -0 "$APP_PID" 2>/dev/null; do sleep 0.25; done
  if kill -0 "$APP_PID" 2>/dev/null; then fail "app still running after Reiniciar agora"; else ok "the app quit"; fi
  if wait_file "$E2E_TMP/relaunched" 10; then ok "the relaunch ran after the app was gone"; else fail "no relaunch"; fi
}
