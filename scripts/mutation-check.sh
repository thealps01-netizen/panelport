#!/usr/bin/env bash
# Mutation check (dev sandbox only): each mutation breaks one security rule; the named unit test
# must fail with it and pass again once the file is restored.
set -uo pipefail
cd "$(dirname "$0")/../src-tauri"
run() { cargo test -q "$1" 2>&1 | grep -E "^test .* (FAILED|ok)$|test result|could call|panicked at" | head -4; }
mutate() { # name, file, python expression transforming s, test filter
  local name="$1" file="$2" expr="$3" filter="$4"
  cp "$file" /tmp/mut.bak
  python3 -c "import sys; p=sys.argv[1]; s=open(p).read(); s2=$expr; assert s2!=s, 'mutation did not apply'; open(p,'w').write(s2)" "$file" || { echo "MUTATION NOT APPLIED: $name"; return; }
  echo "=== RED: $name"
  run "$filter"
  cp /tmp/mut.bak "$file"
  echo "=== GREEN: $name (restored)"
  run "$filter"
}
mutate "title-bar capability gains a remote section" capabilities/title-bar.json \
  's.replace("\"webviews\": [\"bar-*\"],", "\"webviews\": [\"bar-*\"],\n  \"remote\": {\"urls\": [\"http://localhost:*\", \"http://localhost:*/*\", \"https://*\", \"https://*/*\"]},")' \
  ipc_security_tests
mutate "app capability also matches panel windows (pwin-*)" capabilities/local-pages.json \
  's.replace("\"windows\": [\"launcher\"]", "\"windows\": [\"launcher\", \"pwin-*\"]")' \
  panel_origins_get_no_ipc_but_bundled_pages_do
mutate "title bar may navigate anywhere" src/urls.rs \
  's.replace("if is_app_local(target) && target.path() == \"/titlebar.html\" {", "if true {")' \
  title_bar_cannot_leave_its_page
mutate "top-level guard switched off" src/lib.rs \
  's.replace(".map(|u| urls::is_app_local(&u))\n            .unwrap_or(false);", ".is_ok();", 1)' \
  panel_origins_get_no_ipc_but_bundled_pages_do
