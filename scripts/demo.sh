#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="${1:-$ROOT/runs/demo}"
cd "$ROOT"
cargo build --quiet --bin llm-fallback-compatibility-tester
BIN="$ROOT/target/debug/llm-fallback-compatibility-tester"
"$BIN" validate --config examples/support-triage/contract.yaml
set +e
"$BIN" run --config examples/support-triage/contract.yaml --output "$OUT" --include-content
status=$?
set -e
if [[ "$status" -ne 1 ]]; then
  echo "Expected the unsafe fallback to make the run exit 1; got $status" >&2
  exit 1
fi
python3 - "$OUT/run.json" <<'PY'
import json, sys
run=json.load(open(sys.argv[1], encoding="utf-8"))
verdicts={s["id"]:s["verdict"] for s in run["scenarios"]}
assert verdicts.get("safe-fallback")=="passed", verdicts
assert verdicts.get("unsafe-fallback")=="failed", verdicts
print("Verified expected safe pass and unsafe contract failure.")
PY
echo "Report: $OUT/report/index.html"
