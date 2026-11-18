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
js() { req "$(jq -cn --arg l "$1" --arg j "$2" '{op:"eval",label:$l,js:$j}')" >/dev/null; }
# Evaluates a JS expression (or promise) in a window and prints its JSON value, read back
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
  js "$1" "history.replaceState(null, '', location.pathname + location.search)"
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
  [ -n "${XVFB_PID:-}" ] && kill "$XVFB_PID" 2>/dev/null
  cp "$WORK"/data/*/logs/*.log "$OUT/" 2>/dev/null || find "$WORK" -name '*.log' -exec cp {} "$OUT/" \;
  rm -rf "$WORK"
}
trap cleanup EXIT

# ---- environment ------------------------------------------------------------------------
docker compose -f "$ROOT/tests/e2e/compose.yaml" up -d --wait >/dev/null 2>&1 || docker compose -f "$ROOT/tests/e2e/compose.yaml" up -d
python3 -m http.server 8765 --bind 127.0.0.1 --directory "$WORK" >/dev/null 2>&1 &
HTTP_PID=$!
Xvfb :99 -screen 0 1600x1000x24 >/dev/null 2>&1 &
XVFB_PID=$!
export DISPLAY=:99
eval "$(dbus-launch --sh-syntax)"
export XDG_DATA_HOME="$WORK/data" XDG_CONFIG_HOME="$WORK/config" XDG_CACHE_HOME="$WORK/cache"
export PANELPORT_E2E_DIR="$E2E" PANELPORT_LANG="${PANELPORT_LANG:-en}"
for _ in $(seq 1 60); do curl -fs http://localhost:3552/api/health >/dev/null && break; sleep 1; done

"$BIN" >"$OUT/app-stdout.txt" 2>&1 &
APP_PID=$!

# ---- 1. first run: launcher opens Find panels ---------------------------------------------
for _ in $(seq 1 60); do case "$(labels)" in *launcher*) break ;; esac; sleep 0.5; done
check "launcher window opens on first run" grep -q launcher <<<"$(labels)"
for _ in $(seq 1 40); do
  n="$(probe launcher "document.querySelectorAll('#find-body .cand').length")"
  [ "${n:-0}" -gt 0 ] 2>/dev/null && break
  sleep 0.5
done
CANDS="$(probe launcher "[...document.querySelectorAll('#find-body .cand')].map(r => ({url: r.dataset.url, name: r.querySelector('input[type=text]').value, ticked: r.querySelector('input[type=checkbox]').checked, badge: r.querySelector('.badge').textContent}))")"
echo "$CANDS" > "$OUT/candidates.json"
check "discovery finds Arcane (docker) pre-ticked" jq -e 'any(.[]; .url == "http://localhost:3552/" and .ticked and (.badge | test("Docker: arcane")))' <<<"$CANDS"
check "discovery finds whoami container port 8081" jq -e 'any(.[]; .url == "http://localhost:8081/")' <<<"$CANDS"
check "discovery finds plain local web server on 8765" jq -e 'any(.[]; .url == "http://localhost:8765/")' <<<"$CANDS"
check "discovery skips the socket proxy (no published port)" jq -e 'all(.[]; .url != "http://localhost:2375/")' <<<"$CANDS"
shot 01-first-run-find-panels Panelport

# Tick Arcane and whoami only, add them.
js launcher "document.querySelectorAll('#find-body .cand').forEach(r => { r.querySelector('input[type=checkbox]').checked = /:(3552|8081)\\//.test(r.dataset.url); }); updateAddButton(); document.getElementById('find-add').click();"
sleep 3
PANELS="$(probe launcher "invoke('list_panels').then(ps => ps.map(p => ({id: p.id, name: p.name, url: p.url, state: p.state})))")"
echo "$PANELS" > "$OUT/panels.json"
check "two panels added" jq -e 'length == 2' <<<"$PANELS"
ARCANE_ID="$(jq -r '.[] | select(.url == "http://localhost:3552/") | .id' <<<"$PANELS")"
WHOAMI_ID="$(jq -r '.[] | select(.url == "http://localhost:8081/") | .id' <<<"$PANELS")"
for _ in $(seq 1 20); do
  st="$(probe launcher "invoke('list_panels').then(ps => ps.map(p => p.state).join(','))")"
  [ "$st" = '"up,up"' ] && break
  sleep 1
done
check "both panels report up" test "$st" = '"up,up"'
shot 02-launcher-healthy Panelport 3

# The Find panels sheet keeps its buttons on screen with a long candidate list.
FITS="$(probe launcher "(() => { state.candidates = Array.from({length: 25}, (_, i) => ({url: 'http://localhost:' + (9000 + i) + '/', name: 'Test ' + i, port: 9000 + i, container: null, title: null, recommended: false})); renderCandidates(); document.getElementById('find').classList.remove('hidden'); const r = document.getElementById('find-add').getBoundingClientRect(); const ok = r.top >= 0 && r.bottom <= innerHeight; document.getElementById('find').classList.add('hidden'); return ok; })()")"
check "find sheet buttons stay visible with 25 candidates" test "$FITS" = "true"

# Set Arcane's health path to /api/health through the Settings flow's command.
probe launcher "invoke('list_panels').then(ps => invoke('save_panels', {panels: ps.map(p => ({id: p.id, name: p.name, url: p.url, icon: p.icon, health_path: p.url.includes('3552') ? '/api/health' : p.health_path}))})).then(r => r.length)" >/dev/null
check "settings file has schema 1 and the health path" \
  jq -e '.schema == 1 and any(.panels[]; .health_path == "/api/health")' "$(find "$WORK/data" -name settings.json | head -1)"

# ---- 2. panel window shows Arcane --------------------------------------------------------
req "$(jq -cn --arg id "$ARCANE_ID" '{op:"open",id:$id}')" >/dev/null
PL="panel-$ARCANE_ID"
check "Arcane window shows http://localhost:3552" wait_url "$PL" "http://localhost:3552/" 30
sleep 4
shot 03-arcane-panel-window Arcane 2

# ---- 3. the panel origin gets no IPC -----------------------------------------------------
IPC="$(probe "$PL" "(window.__TAURI_INTERNALS__ ? window.__TAURI_INTERNALS__.invoke('list_panels').then(() => 'ALLOWED', e => 'denied: ' + e) : 'no-bridge')")"
echo "list_panels from Arcane origin: $IPC" | tee -a "$OUT/ipc-probe.txt"
check "Arcane origin cannot call list_panels" grep -qv ALLOWED <<<"$IPC"
IPC2="$(probe "$PL" "(window.__TAURI_INTERNALS__ ? window.__TAURI_INTERNALS__.invoke('start_containers', {id: '$ARCANE_ID'}).then(() => 'ALLOWED', e => 'denied: ' + e) : 'no-bridge')")"
echo "start_containers from Arcane origin: $IPC2" | tee -a "$OUT/ipc-probe.txt"
check "Arcane origin cannot call start_containers" grep -qv ALLOWED <<<"$IPC2"
IPC3="$(probe "$PL" "(window.__TAURI_INTERNALS__ ? window.__TAURI_INTERNALS__.invoke('plugin:opener|open_url', {url: 'https://example.com'}).then(() => 'ALLOWED', e => 'denied: ' + e) : 'no-bridge')")"
echo "plugin:opener|open_url from Arcane origin: $IPC3" | tee -a "$OUT/ipc-probe.txt"
check "Arcane origin cannot call the opener plugin" grep -qv ALLOWED <<<"$IPC3"

# ---- 4. navigation guard -----------------------------------------------------------------
js "$PL" "location.href = 'http://localhost:8081/'"
sleep 2
check "navigating the Arcane window to another origin is blocked" wait_url "$PL" "http://localhost:3552/" 3
js "$PL" "location.href = 'file:///etc/passwd'"
sleep 1
check "file: navigation is blocked" wait_url "$PL" "http://localhost:3552/" 3

# A panel page must not load or frame the bundled pages, which hold the IPC rights.
APPLOG="$(find "$WORK/data" -name 'panelport*.log' | head -1)"
BEFORE="$(grep -c 'discovery found' "$APPLOG")"
js "$PL" "location.href = 'tauri://localhost/launcher.html#find'"
sleep 2
check "panel window cannot navigate to the bundled launcher" wait_url "$PL" "http://localhost:3552/" 3
js "$PL" "const f = document.createElement('iframe'); f.src = 'tauri://localhost/launcher.html#find'; document.body.appendChild(f);"
sleep 5
AFTER="$(grep -c 'discovery found' "$APPLOG")"
check "a framed bundled page gets no IPC (no discovery run: $BEFORE -> $AFTER)" test "$BEFORE" = "$AFTER"

# ---- 5. Arcane containers stopped -> down page -> Start containers ------------------------
docker stop arcane arcane-docker-proxy >/dev/null
check "Arcane window switches to the not-reachable page" wait_url "$PL" "down.html?panel=$ARCANE_ID" 40
sleep 3
DETAIL="$(probe "$PL" "invoke('panel_detail', {id: '$ARCANE_ID'}).then(d => ({local: d.local, engine: d.engine, containers: d.containers, state: d.panel.state}))")"
echo "$DETAIL" > "$OUT/detail-arcane-stopped.json"
# The whole compose project is listed, dependencies first (the proxy before Arcane).
check "down page lists the compose project's containers, dependency first" \
  jq -e '.engine == "running" and (.containers | map(.name) | index("arcane-docker-proxy")) < (.containers | map(.name) | index("arcane"))' <<<"$DETAIL"
check "Start containers button shown" grep -q true <<<"$(probe "$PL" "!!document.getElementById('start-containers')")"
shot 04-arcane-stopped-down-page Arcane 1
js "$PL" "document.getElementById('start-containers').click()"
check "Arcane comes back after Start containers" wait_url "$PL" "http://localhost:3552/" 90
check "both containers running again" test "$(docker inspect -f '{{.State.Running}}' arcane arcane-docker-proxy | sort -u)" = "true"
sleep 4
shot 05-arcane-recovered Arcane 1

# ---- 6. Docker daemon stopped -> engine not running -> recovery ---------------------------
DOCKERD_PID="$(pgrep -x dockerd | head -1)"
sudo kill -TERM "$DOCKERD_PID"
for _ in $(seq 1 30); do pgrep -x dockerd >/dev/null || break; sleep 1; done
check "Arcane window switches to the not-reachable page with Docker down" wait_url "$PL" "down.html?panel=$ARCANE_ID" 40
sleep 12
DETAIL2="$(probe "$PL" "invoke('panel_detail', {id: '$ARCANE_ID'}).then(d => ({local: d.local, engine: d.engine, containers: d.containers, state: d.panel.state}))")"
echo "$DETAIL2" > "$OUT/detail-docker-down.json"
check "down page reports the Docker engine as not running" jq -e '.engine == "not_running"' <<<"$DETAIL2"
shot 06-docker-daemon-down Arcane 1
sudo setsid nohup dockerd >/tmp/dockerd-e2e.log 2>&1 < /dev/null &
for _ in $(seq 1 60); do docker info >/dev/null 2>&1 && break; sleep 1; done
sleep 12
check "after Docker is back, Start containers is offered" grep -q true <<<"$(probe "$PL" "!!document.getElementById('start-containers')")"
js "$PL" "document.getElementById('start-containers').click()"
check "Arcane comes back after Docker restart" wait_url "$PL" "http://localhost:3552/" 90
docker start test-whoami >/dev/null 2>&1

# ---- 7. single instance ------------------------------------------------------------------
timeout 20 "$BIN" >/dev/null 2>&1
SECOND=$?
check "second launch exits (single instance), exit code $SECOND" test "$SECOND" -eq 0
check "first instance still running" kill -0 "$APP_PID"

# ---- 8. settings window --------------------------------------------------------------------
js launcher "invoke('show_settings')"
sleep 3
shot 07-settings "Panelport Settings" 1

# ---- logs ------------------------------------------------------------------------------------
LOG="$(find "$WORK/data" -name 'panelport*.log' | head -1)"
check "log file exists under the per-user data folder" test -s "$LOG"
cp "$LOG" "$OUT/panelport.log" 2>/dev/null
echo
echo "$(grep -c ^PASS "$RESULTS") passed, $(grep -c ^FAIL "$RESULTS") failed"
! grep -q ^FAIL "$RESULTS"
