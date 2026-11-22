#!/usr/bin/env bash
# Runs the checks of .github/workflows/ci.yml on this machine (this OS only),
# for example before opening a pull request:
#   bash scripts/ci-local.sh            fmt, clippy, lint, tests, conformance
#   bash scripts/ci-local.sh --python   also build the wheel and run pytest
#                                       (in a throwaway virtualenv; needs
#                                       maturin and network access for numpy,
#                                       pytest and qiskit)
set -euo pipefail
cd "$(dirname "$0")/.."
step() { printf '\n=== %s ===\n' "$1"; }

step "fmt"
cargo fmt --all -- --check
step "clippy"
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo clippy -p vikshep-quantum-py --features python --all-targets --locked -- -D warnings
step "libm without optional features"
if cargo tree --locked -e features -i libm | grep -E 'libm feature'; then
    echo "libm must be built with no features"
    exit 1
fi
step "determinism lint"
scripts/determinism-lint.sh
step "tests"
cargo test --workspace --locked

step "conformance (default threads and one thread)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cargo run --release --locked -q -p vikshep-qconformance -- run --report "$tmp/report.json"
cargo run --release --locked -q -p vikshep-qconformance -- run --threads 1 --report "$tmp/report-1.json"
cmp "$tmp/report.json" "$tmp/report-1.json"
echo "reports identical; SHA-256 $(shasum -a 256 "$tmp/report.json" | cut -d' ' -f1)"

if [ "${1:-}" = "--python" ]; then
    step "wheel and pytest"
    python3 -m venv "$tmp/venv"
    "$tmp/venv/bin/pip" install -q numpy pytest qiskit
    maturin build --release --locked -m crates/vikshep-quantum-py/Cargo.toml -o "$tmp/dist"
    "$tmp/venv/bin/pip" install -q --no-index --find-links "$tmp/dist" vikshep-quantum
    (cd "$tmp" && "$tmp/venv/bin/python" -I -m pytest -q "$OLDPWD/crates/vikshep-quantum-py/tests")
    step "seeded Qiskit workloads (run twice: the records must be identical)"
    (cd "$tmp" && "$tmp/venv/bin/python" -I "$OLDPWD/scripts/qiskit-record.py" "$tmp/record-1.json")
    (cd "$tmp" && "$tmp/venv/bin/python" -I "$OLDPWD/scripts/qiskit-record.py" "$tmp/record-2.json")
    cmp "$tmp/record-1.json" "$tmp/record-2.json"
fi
printf '\nALL LOCAL CHECKS PASSED\n'
