# Contributing

## Contributor License Agreement

Outside contributions require a signed Contributor License Agreement (CLA),
because the code is offered under both the AGPL and a commercial licence
(see [`LICENSING.md`](LICENSING.md)). No CLA has been published yet. Until
one is, pull requests from outside contributors will not be merged.

Bug reports, questions about the specification, and reports of
cross-platform mismatches are welcome as issues at any time.

## Development checks

Run these before opening a pull request. CI runs the same checks, and
`bash scripts/ci-local.sh --python` runs all of them.

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo clippy -p vikshep-quantum-py --features python --all-targets --locked -- -D warnings
cargo test --workspace --locked
scripts/determinism-lint.sh
cargo run --release --locked -p vikshep-qconformance -- run
cargo run --release --locked -p vikshep-qconformance -- run --threads 1
```

The two conformance reports must be identical.

Python package (needs a Rust toolchain, Python 3.10 or later and maturin):

```sh
maturin build --release --locked -m crates/vikshep-quantum-py/Cargo.toml -o dist
python -m pip install numpy pytest qiskit
python -m pip install --no-index --find-links dist vikshep-quantum
python -I -m pytest -q crates/vikshep-quantum-py/tests
```

## Numerics rules

These follow from [`spec/VQS-1.md`](spec/VQS-1.md) and the VDS-1 rules it
inherits:

- **No platform maths or fused operations.**
  - Never call `f32`/`f64` transcendental or power methods (`exp`, `ln`,
    `sin`, `cos`, `powf`, `powi`, `hypot`, ...). Use `vikshep-detmath`.
    `sqrt` is allowed.
  - Never use `mul_add` or fast/algebraic float intrinsics.
  - Do not enable `clippy::suboptimal_flops`.
- **Tier-1 arithmetic goes through `vikshep_qcore::arith`.** Every
  operation is flushed, and zero matrix entries are never skipped (VQS-1
  section 5.3).
- **Reductions have a fixed order that does not depend on the thread
  count.** Sums over the state use the pairwise tree of VQS-1 section 7.
- **Bit-changing changes need a version bump.** Any change that alters an
  output bit requires a new `vqs_version` or conformance suite and new
  vectors (`vikshep-qconformance generate`). Examples:
  - arithmetic order;
  - gate matrices;
  - sampling;
  - constants;
  - the pinned dependency versions.

  Within one suite, cases may be added but existing expected outputs must
  not change.
- **Bindings add no arithmetic.** They copy inputs into contiguous
  row-major buffers and call the reference code.
- **Lint exceptions** go in `scripts/determinism-lint.allow`, each with a
  justification.
- **No licence checks, telemetry or network calls** in this repository.

## Style

No AI-attribution text in files, commits or pull requests. Stage explicit
paths only.
