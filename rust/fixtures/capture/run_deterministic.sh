#!/usr/bin/env bash
# Run the no-media capture scripts. Intended to be run from inside the nix
# devshell with PYTHONPATH set to "<repo>:<repo>/rust/fixtures/capture".
set -euo pipefail
cd "$(dirname "$0")"
for s in capture_enums capture_lang capture_paths capture_config; do
  echo "=== $s ==="
  python "$s.py"
done
echo "deterministic fixtures captured."
