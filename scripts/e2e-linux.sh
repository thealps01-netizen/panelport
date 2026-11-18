#!/usr/bin/env bash
# End-to-end run of a real Panelport build against real panels, for a disposable Linux VM/sandbox
# only: it stops and starts Docker containers and the Docker daemon itself.
#
# Needs: Xvfb, dbus-launch, xdotool, ImageMagick `import`, jq, python3, docker (own daemon).
# Build first: (cd src-tauri && cargo build --features e2e)
# Usage: scripts/e2e-linux.sh <output dir>
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="${1:?output dir}"
mkdir -p "$OUT"
OUT="$(cd "$OUT" && pwd)"
BIN="$ROOT/src-tauri/target/debug/panelport"
RESULTS="$OUT/results.txt"
: > "$RESULTS"
WORK="$(mktemp -d)"
E2E="$WORK/e2e"
mkdir -p "$E2E"
REQ=0

pass() { echo "PASS $*" | tee -a "$RESULTS"; }
fail() { echo "FAIL $*" | tee -a "$RESULTS"; }
check() { local name="$1"; shift; if "$@"; then pass "$name"; else fail "$name"; fi; }

req() {
  REQ=$((REQ + 1))
  local n="r$REQ"
  printf '%s' "$1" > "$E2E/$n.tmp" && mv "$E2E/$n.tmp" "$E2E/$n.json"
  for _ in $(seq 1 150); do
    if [ -f "$E2E/$n.out" ]; then cat "$E2E/$n.out"; rm -f "$E2E/$n.out"; return 0; fi
    sleep 0.1
  done
  echo "TIMEOUT"
}
url_of() { req "$(jq -cn --arg l "$1" '{op:"url",label:$l}')"; }
labels() { req '{"op":"labels"}'; }
winfo() { req "$(jq -cn --arg l "$1" '{op:"winfo",label:$l}')"; }
size() { req "$(jq -cn --arg l "$1" --argjson w "$2" --argjson h "$3" '{op:"size",label:$l,w:$w,h:$h}')" >/dev/null; }
js() { req "$(jq -cn --arg l "$1" --arg j "$2" '{op:"eval",label:$l,js:$j}')" >/dev/null; }
# Evaluates a JS expression (or promise) in a webview and prints its JSON value, read back
# through the URL fragment (the harness has no other return channel by design).
probe() {
  local wrapped="Promise.resolve().then(() => ($2)).then(v => { location.hash = 'e2e=' + encodeURIComponent(JSON.stringify(v)); }, e => { location.hash = 'e2e=' + encodeURIComponent(JSON.stringify({ error: String((e && e.message) || e) })); })"
  js "$1" "$wrapped"
  local u=""
  for _ in $(seq 1 30); do
    sleep 0.2
    u="$(url_of "$1")"
    case "$u" in *"#e2e="*) break ;; esac
  done
  python3 -c 'import sys, urllib.parse as p; u = sys.argv[1]; print(p.unquote(u.split("#e2e=", 1)[1]) if "#e2e=" in u else "NOHASH " + u)' "$u"
  js "$1" "history.replaceState(null, '', location.pathname + location.search + (location.hash.startsWith('#e2e=') ? '' : location.hash))"
}
wait_url() { # label, substring, seconds
  local u=""
  for _ in $(seq 1 $(( $3 * 2 ))); do
    u="$(url_of "$1")"
    case "$u" in *"$2"*) echo "$u"; return 0 ;; esac
    sleep 0.5
  done
  echo "$u"
  return 1
}
shot() { # name window-title [delay]: raise the window to the top-left corner, crop it from the screen
  sleep "${3:-1.5}"
  local id
  id="$(xdotool search --onlyvisible --name "^$2\$" 2>/dev/null | tail -1)"
  if [ -z "$id" ]; then import -window root "$OUT/$1.png"; return; fi
  xdotool windowmove "$id" 0 0 windowraise "$id"
  sleep 0.7
  eval "$(xdotool getwindowgeometry --shell "$id")"
  import -window root -crop "${WIDTH}x${HEIGHT}+0+0" +repage "$OUT/$1.png"
}

cleanup() {
  [ -n "${APP_PID:-}" ] && kill "$APP_PID" 2>/dev/null
  [ -n "${HTTP_PID:-}" ] && kill "$HTTP_PID" 2>/dev/null
  [ -n "${API_PID:-}" ] && kill "$API_PID" 2>/dev/null
  [ -n "${XVFB_PID:-}" ] && kill "$XVFB_PID" 2>/dev/null
  rm -rf "$WORK"
}
trap cleanup EXIT

# ---- environment ------------------------------------------------------------------------
docker compose -f "$ROOT/tests/e2e/compose.yaml" up -d --wait >/dev/null 2>&1 || docker compose -f "$ROOT/tests/e2e/compose.yaml" up -d
python3 -m http.server 8765 --bind 127.0.0.1 --directory "$WORK" >/dev/null 2>&1 &
HTTP_PID=$!
# A local program API that answers 401 in plain text (like the cua daemon): not a panel.
python3 "$ROOT/tests/e2e/api401.py" 62310 >/dev/null 2>&1 &
API_PID=$!
Xvfb :99 -screen 0 1600x1000x24 >/dev/null 2>&1 &
XVFB_PID=$!
export DISPLAY=:99
eval "$(dbus-launch --sh-syntax)"
export XDG_DATA_HOME="$WORK/data" XDG_CONFIG_HOME="$WORK/config" XDG_CACHE_HOME="$WORK/cache"
export PANELPORT_E2E_DIR="$E2E" PANELPORT_LANG="${PANELPORT_LANG:-en}"
# Dark like the owner's Windows; PANELPORT_THEME=light renders the light counterpart.
if [ "${PANELPORT_THEME:-dark}" = dark ]; then export GTK_THEME=Adwaita:dark; else export GTK_THEME=Adwaita; fi
for _ in $(seq 1 60); do curl -fs http://localhost:3552/api/health >/dev/null && break; sleep 1; done

"$BIN" >"$OUT/app-stdout.txt" 2>&1 &
APP_PID=$!

# ---- 1. first run: the launcher opens on Find panels --------------------------------------
for _ in $(seq 1 60); do case "$(labels)" in *launcher*) break ;; esac; sleep 0.5; done
check "launcher window opens on first run" grep -q launcher <<<"$(labels)"
check "launcher has a single webview (frameless window)" jq -e '.webviews == ["launcher"]' <<<"$(winfo launcher)"
size launcher 1440 900
for _ in $(seq 1 60); do
  st="$(probe launcher "L.find.state")"
  [ "$st" = '"done"' ] && break
  sleep 0.5
done
check "first run shows the Find panels view" test "$(probe launcher "L.view")" = '"find"'
CANDS="$(probe launcher "L.find.candidates.map(c => ({url: c.url, name: c.name, ticked: !!L.find.ticked[c.url], container: c.container, known: c.known}))")"
SKIPPED="$(probe launcher "L.find.skipped")"
echo "$CANDS" > "$OUT/candidates.json"; echo "$SKIPPED" > "$OUT/skipped.json"
check "discovery finds Arcane (docker) pre-ticked" jq -e 'any(.[]; .url == "http://localhost:3552/" and .ticked and .container == "arcane")' <<<"$CANDS"
check "discovery finds the nginx container web page on 8082, pre-ticked" jq -e 'any(.[]; .url == "http://localhost:8082/" and .ticked)' <<<"$CANDS"
check "discovery finds the plain local web server on 8765 (not ticked)" jq -e 'any(.[]; .url == "http://localhost:8765/" and (.ticked | not))' <<<"$CANDS"
check "discovery skips the socket proxy (no published port)" jq -e 'all(.[]; .url != "http://localhost:2375/")' <<<"$CANDS"
check "the 401 plain-text API port is listed as skipped (401)" jq -e 'any(.[]; .port == 62310 and .reason == "401")' <<<"$SKIPPED"
check "whoami (plain text) is listed as skipped, not offered" jq -e 'any(.[]; .port == 8081)' <<<"$SKIPPED"
check "the skipped line is shown" grep -q 'true' <<<"$(probe launcher "!!document.querySelector('#find-rows .skip') && document.querySelector('#find-rows .skip').textContent.includes('62310')")"
shot 01-find-panels-1440 Panelport 1

js launcher "L.find.ticked = {'http://localhost:3552/': true, 'http://localhost:8082/': true}; renderFind(); document.getElementById('find-add').click();"
sleep 3
PANELS="$(probe launcher "invoke('list_panels').then(ps => ps.map(p => ({id: p.id, name: p.name, url: p.url, state: p.state})))")"
echo "$PANELS" > "$OUT/panels.json"
check "two panels added" jq -e 'length == 2' <<<"$PANELS"
ARCANE_ID="$(jq -r '.[] | select(.url == "http://localhost:3552/") | .id' <<<"$PANELS")"
WEB_ID="$(jq -r '.[] | select(.url == "http://localhost:8082/") | .id' <<<"$PANELS")"
check "after adding, the launcher shows the Panels view" test "$(probe launcher "L.view")" = '"panels"'
for _ in $(seq 1 20); do
  st="$(probe launcher "invoke('list_panels').then(ps => ps.map(p => p.state).join(','))")"
  [ "$st" = '"up,up"' ] && break
  sleep 1
done
check "both panels report up" test "$st" = '"up,up"'
js launcher "select('$ARCANE_ID')"
sleep 2
check "stage shows the selected panel's name" grep -qi 'arcane' <<<"$(probe launcher "document.querySelector('#stage .big').textContent")"
check "panel names render with lang=en (no dotted İ)" test "$(probe launcher "document.querySelector('#stage .big').lang")" = '"en"'
check "the list shows each panel's state in words" grep -q 'Running' <<<"$(probe launcher "[...document.querySelectorAll('#list .state')].map(s => s.textContent).join('|')")"
shot 02-launcher-1440 Panelport 2
size launcher 1000 700
shot 02-launcher-1000 Panelport 2
check "the stage name fits at 1000x700" test "$(probe launcher "(() => { const b = document.querySelector('#stage .big'); return b.scrollWidth <= b.clientWidth + 1; })()")" = "true"
size launcher 1440 900

# Arcane's health path through the save command (as the Settings editor does).
probe launcher "invoke('list_panels').then(ps => invoke('save_panels', {panels: ps.map(p => ({id: p.id, name: p.name, url: p.url, icon: p.icon, health_path: p.url.includes('3552') ? '/api/health' : p.health_path}))})).then(r => r.length)" >/dev/null
check "settings file has schema 1, the health path and the 1.1 prefs" \
  jq -e '.schema == 1 and any(.panels[]; .health_path == "/api/health") and .prefs.keep_previews == true' "$(find "$WORK/data" -name settings.json | head -1)"

# ---- 2. Not a web page: shown as its own state -------------------------------------------
probe launcher "invoke('list_panels').then(ps => invoke('save_panels', {panels: ps.map(p => ({id: p.id, name: p.name, url: p.url, icon: p.icon, health_path: p.health_path})).concat([{id: '', name: 'Daemon API', url: 'http://localhost:62310/', icon: 'monogram', health_path: '/'}])})).then(r => r.length)" >/dev/null
for _ in $(seq 1 20); do
  NP="$(probe launcher "invoke('list_panels').then(ps => ps.filter(p => p.url.includes('62310')).map(p => p.state + '|' + p.detail)[0])")"
  case "$NP" in *not_page*) break ;; esac
  sleep 1
done
check "a 401 plain-text panel is shown as not a web page ($NP)" grep -q 'not_page|HTTP 401, text/plain' <<<"$NP"
sleep 2
ITEM="$(probe launcher "[...document.querySelectorAll('#list .item')].filter(i => i.textContent.includes('Daemon')).map(i => i.querySelector('.state').textContent)[0]")"
check "list item says 'Not a web page / no access' ($ITEM)" grep -q 'Not a web page / no access' <<<"$ITEM"
probe launcher "invoke('list_panels').then(ps => invoke('save_panels', {panels: ps.filter(p => !p.url.includes('62310')).map(p => ({id: p.id, name: p.name, url: p.url, icon: p.icon, health_path: p.health_path}))})).then(r => r.length)" >/dev/null
check "bundled text inputs opt out of autofill" test "$(probe launcher "[...document.querySelectorAll('input[type=text]')].every(i => i.autocomplete === 'off')")" = "true"

# ---- 3. Settings and Find views ------------------------------------------------------------
js launcher "show('settings')"
sleep 2
check "settings lists both panels with a drag grip" test "$(probe launcher "document.querySelectorAll('#sec-panels .pl .grip').length")" = "2"
check "settings has all five sections" test "$(probe launcher "['sec-panels','sec-general','sec-notifications','sec-passwords','sec-about'].every(id => document.getElementById(id))")" = "true"
shot 03-settings-1440 Panelport 1
# Reorder with the keyboard path (Alt+Down on the first grip) and check the saved order.
FIRST="$(probe launcher "S_first = document.querySelector('#sec-panels .pl').dataset.id")"
js launcher "document.querySelector('#sec-panels .pl .grip').dispatchEvent(new KeyboardEvent('keydown', {key: 'ArrowDown', altKey: true, bubbles: true}))"
sleep 2
check "Alt+Down moves a panel down in the saved order" test "$(probe launcher "invoke('list_panels').then(ps => ps[1].id)")" = "$FIRST"
js launcher "show('find')"
sleep 4
shot 04-find-1000 Panelport 1
js launcher "show('panels')"

# ---- 4. Panel window: two webviews, title bar, IPC ----------------------------------------
req "$(jq -cn --arg id "$ARCANE_ID" '{op:"open",id:$id}')" >/dev/null
PL="panel-$ARCANE_ID"; BL="bar-$ARCANE_ID"; WL="pwin-$ARCANE_ID"
check "Arcane panel webview shows http://localhost:3552" wait_url "$PL" "http://localhost:3552/" 30
INFO="$(winfo "$WL")"
echo "$INFO" > "$OUT/panel-window.json"
check "panel window holds two webviews: title bar and panel" jq -e --arg b "$BL" --arg p "$PL" '(.webviews | sort) == ([$b, $p] | sort)' <<<"$INFO"
check "title bar webview shows its bundled page" wait_url "$BL" "titlebar.html" 10
sleep 3
check "title bar shows the panel name" grep -qi arcane <<<"$(probe "$BL" "document.getElementById('name').textContent")"
size "$WL" 1440 900
shot 05-arcane-panel-window-1440 Arcane 3

for cmd in list_panels start_containers bar_info bar_back win_close win_toggle_maximize "plugin:opener|open_url" "plugin:webview|create_webview_window"; do
  R="$(probe "$PL" "(window.__TAURI_INTERNALS__ ? window.__TAURI_INTERNALS__.invoke('$cmd', {id: '$ARCANE_ID', url: 'https://example.com'}).then(() => 'ALLOWED', e => 'denied: ' + e) : 'no-bridge')")"
  echo "$cmd from the Arcane origin: $R" | tee -a "$OUT/ipc-probe.txt"
  check "Arcane origin in the two-webview window cannot call $cmd" grep -qv ALLOWED <<<"$R"
done
check "Arcane window still open after its page tried win_close" jq -e '.webviews | length == 2' <<<"$(winfo "$WL")"

# The title bar may not leave its page, not even to another bundled page.
js "$BL" "location.href = 'http://localhost:3552/'"
sleep 2
check "title bar cannot navigate to a panel origin" wait_url "$BL" "titlebar.html" 3
js "$BL" "location.href = 'tauri://localhost/launcher.html'"
sleep 2
check "title bar cannot navigate to another bundled page" wait_url "$BL" "titlebar.html" 3
R="$(probe "$BL" "invoke('list_panels').then(() => 'ALLOWED', e => 'denied: ' + (e && e.message || e))")"
echo "list_panels from the title bar: $R" | tee -a "$OUT/ipc-probe.txt"
check "title bar cannot call app commands" grep -qv ALLOWED <<<"$R"
check "title bar window control works (maximize)" test "$(probe "$BL" "invoke('win_toggle_maximize')")" = "true"
# Xvfb has no window manager, so GTK cannot carry out the maximize itself; the window state is
# checked on the Windows runner (scripts/screens-windows.ps1).
probe "$BL" "invoke('win_toggle_maximize')" >/dev/null

# ---- 5. navigation guard of the panel webview ---------------------------------------------
js "$PL" "location.href = 'http://localhost:8082/'"
sleep 2
check "navigating the Arcane webview to another origin is blocked" wait_url "$PL" "http://localhost:3552/" 3
js "$PL" "location.href = 'file:///etc/passwd'"
sleep 1
check "file: navigation is blocked" wait_url "$PL" "http://localhost:3552/" 3
APPLOG="$(find "$WORK/data" -name 'panelport*.log' | head -1)"
BEFORE="$(grep -c 'discovery found' "$APPLOG")"
js "$PL" "location.href = 'tauri://localhost/launcher.html#find'"
sleep 2
check "panel webview cannot navigate to the bundled launcher" wait_url "$PL" "http://localhost:3552/" 3
js "$PL" "const f = document.createElement('iframe'); f.src = 'tauri://localhost/launcher.html#find'; document.body.appendChild(f);"
sleep 5
AFTER="$(grep -c 'discovery found' "$APPLOG")"
check "a framed bundled page gets no IPC (no discovery run: $BEFORE -> $AFTER)" test "$BEFORE" = "$AFTER"

# ---- 6. Arcane containers stopped -> down page -> Start containers ------------------------
docker stop arcane arcane-docker-proxy >/dev/null
check "Arcane webview switches to the not-reachable page" wait_url "$PL" "down.html?panel=$ARCANE_ID" 40
sleep 4
DETAIL="$(probe "$PL" "invoke('panel_detail', {id: '$ARCANE_ID'}).then(d => ({engine: d.engine, version: d.engine_version, containers: d.containers.map(c => ({name: c.name, owner: c.owner, running: c.running, exit_code: c.exit_code})), project: d.project, state: d.panel.state}))")"
echo "$DETAIL" > "$OUT/detail-arcane-stopped.json"
check "down page lists the compose project's containers, dependency first" \
  jq -e '.engine == "running" and (.containers | map(.name) | index("arcane-docker-proxy")) < (.containers | map(.name) | index("arcane"))' <<<"$DETAIL"
check "down page knows the owner container, its exit code and the engine version" \
  jq -e '(.containers[] | select(.owner) | .name) == "arcane" and (.containers[] | select(.owner) | .exit_code) != null and .version != null' <<<"$DETAIL"
check "Start containers button shown" grep -q true <<<"$(probe "$PL" "!!document.getElementById('start-containers')")"
sleep 6
check "down page shows the container's last log lines" grep -q true <<<"$(probe "$PL" "(document.querySelector('.log') || {}).textContent.length > 20")"
js launcher "select('$ARCANE_ID')"
sleep 3
check "launcher list offers Start containers inline for the stopped panel" grep -q true <<<"$(probe launcher "!!document.querySelector('#opt-$ARCANE_ID .act')")"
shot 06-launcher-arcane-down-1440 Panelport 1
shot 07-down-page-1440 Arcane 1
size "$WL" 1000 700
shot 07-down-page-1000 Arcane 2
size "$WL" 1440 900
js "$PL" "document.getElementById('start-containers').click()"
check "Arcane comes back after Start containers" wait_url "$PL" "http://localhost:3552/" 90
check "both containers running again" test "$(docker inspect -f '{{.State.Running}}' arcane arcane-docker-proxy | sort -u)" = "true"
sleep 3

# ---- 7. Logs drawer in the launcher -------------------------------------------------------
js launcher "toggleLogs('$ARCANE_ID')"
sleep 4
check "launcher log drawer shows the container log" grep -q true <<<"$(probe launcher "(document.getElementById('logs-pre') || {}).textContent.length > 20")"
shot 08-launcher-logs-1440 Panelport 1
js launcher "toggleLogs('$ARCANE_ID')"

# ---- 8. Docker daemon stopped -> engine not running -> recovery ---------------------------
DOCKERD_PID="$(pgrep -x dockerd | head -1)"
sudo kill -TERM "$DOCKERD_PID"
for _ in $(seq 1 30); do pgrep -x dockerd >/dev/null || break; sleep 1; done
check "Arcane webview switches to the not-reachable page with Docker down" wait_url "$PL" "down.html?panel=$ARCANE_ID" 40
sleep 12
DETAIL2="$(probe "$PL" "invoke('panel_detail', {id: '$ARCANE_ID'}).then(d => ({engine: d.engine}))")"
echo "$DETAIL2" > "$OUT/detail-docker-down.json"
check "down page reports the Docker engine as not running" jq -e '.engine == "not_running"' <<<"$DETAIL2"
check "title bar dock line says Docker is not running" grep -qi 'not running' <<<"$(probe launcher "document.getElementById('dock').textContent")"
shot 09-docker-down-1440 Arcane 1
sudo setsid nohup dockerd >/tmp/dockerd-e2e.log 2>&1 < /dev/null &
for _ in $(seq 1 60); do docker info >/dev/null 2>&1 && break; sleep 1; done
sleep 12
check "after Docker is back, Start containers is offered" grep -q true <<<"$(probe "$PL" "!!document.getElementById('start-containers')")"
js "$PL" "document.getElementById('start-containers').click()"
check "Arcane comes back after Docker restart" wait_url "$PL" "http://localhost:3552/" 90
docker start test-whoami test-web >/dev/null 2>&1

# ---- 9. single instance, empty state -------------------------------------------------------
timeout 20 "$BIN" >/dev/null 2>&1
SECOND=$?
check "second launch exits (single instance), exit code $SECOND" test "$SECOND" -eq 0
check "first instance still running" kill -0 "$APP_PID"

# ---- logs ------------------------------------------------------------------------------------
LOG="$(find "$WORK/data" -name 'panelport*.log' | head -1)"
check "log file exists under the per-user data folder" test -s "$LOG"
cp "$LOG" "$OUT/panelport.log" 2>/dev/null
echo
echo "$(grep -c ^PASS "$RESULTS") passed, $(grep -c ^FAIL "$RESULTS") failed"
! grep -q ^FAIL "$RESULTS"
