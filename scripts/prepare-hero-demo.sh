#!/usr/bin/env bash
set -euo pipefail

demo_root="/tmp/graveyard-demo-hero"

rm -rf "$demo_root"
mkdir -p "$demo_root"

cat > "$demo_root/scan.txt" <<'EOF'
CONFIDENCE  TAG               AGE         LOCATION                  FQN
0.86        dead              1.1 years   ./python/app.py:1         python/app.py::dead_python
0.86        dead              1.1 years   ./src/main.rs:1           src/main.rs::old_dead
0.86        dead              1.1 years   ./src/main.rs:2           src/main.rs::brand_new
0.86        dead              1.1 years   ./web/app.js:1            web/app.js::deadJs
Found 4 dead symbol(s) - min-confidence 0.8, min-age 30 days
EOF

cat > "$demo_root/diff.txt" <<'EOF'
CONFIDENCE  TAG               AGE         LOCATION                  FQN
0.86        dead              1.1 years   ./src/main.rs:2           src/main.rs::brand_new
Found 1 dead symbol(s) - min-confidence 0.5, min-age none
EOF

cat > "$demo_root/.graveyard-baseline.json" <<'EOF'
{"version":1}
EOF

cat > "$demo_root/graveyard" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

if [[ "${1:-}" == "scan" ]]; then
  cat "$root/scan.txt"
  exit 0
fi

if [[ "${1:-}" == "baseline" && "${2:-}" == "diff" ]]; then
  cat "$root/diff.txt"
  exit 1
fi

echo "graveyard demo shim: unsupported command: $*" >&2
exit 2
EOF

chmod +x "$demo_root/graveyard"

printf '%s\n' "$demo_root"
