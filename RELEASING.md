# Releasing

A release publishes the open code in three forms, all from one version tag:

| Form | Where | Install |
|---|---|---|
| Python package `vikshep-quantum` (abi3 wheels, Python 3.10+, five targets) | PyPI | `pip install vikshep-quantum` |
| Rust crates `vikshep-qcore` and `vikshep-statevec` | crates.io | `cargo add vikshep-statevec` |
| Wheels and `SHA256SUMS` | GitHub release | download |

Everything is built by `.github/workflows/release.yml`. A pull request that
touches the release files, or a manual run (Actions, Release, Run workflow),
builds and tests all of it and publishes nothing. Only a `vX.Y.Z` tag
publishes, and only after every build succeeded.

## Not published to crates.io

- **`vikshep-qconformance`** embeds the conformance vectors from
  `conformance/`, which is outside its crate directory, so `cargo package`
  cannot include them. It keeps `publish.workspace = true` (the workspace
  default, `publish = false`). The suite runs from a checkout
  (`cargo run --release -p vikshep-qconformance -- run`) and from the
  Python package (`vikshep_quantum.conformance_selftest()`).
- **`vikshep-quantum-py`** is the Python package.
- **New crates.** A crate added later is unpublished until it drops
  `publish.workspace = true`.

No source distribution goes to PyPI: the Python package is built from the
workspace, which an sdist of `crates/vikshep-quantum-py` alone does not
contain. Platforms without a wheel build from a checkout (README,
Quickstart: Python).

## One-time setup

1. **PyPI** (trusted publishing, no stored token): sign in at pypi.org, go
   to Account, Publishing, and choose "Add a new pending publisher". Enter:
   - project `vikshep-quantum`;
   - owner `samvardhan03`;
   - repository `vikshep-quantum`;
   - workflow `release.yml`;
   - environment `pypi`.
2. **GitHub environments:** in Settings, Environments, create `pypi` and
   `crates-io`. Adding yourself as a required reviewer on both makes every
   publish wait for your approval. Restrict both to tags matching `v*`.
3. **crates.io:** sign in with GitHub at crates.io, go to Account Settings,
   API Tokens, and choose New Token:
   - scopes `publish-new` and `publish-update`;
   - crate pattern `vikshep-*`;
   - an expiry.

   Store it as the secret `CARGO_REGISTRY_TOKEN` of the `crates-io`
   environment, not as a repository secret. Once the crates exist, crates.io
   trusted publishing can replace the token.

## Procedure

1. **Bump the version on a branch.**
   - Set the new version in `[workspace.package]` of `Cargo.toml` and in
     every internal pin (`version = "=X.Y.Z"` in `[workspace.dependencies]`).
   - Run `cargo update --workspace`.
   - Update STATUS.md.
   - A version that changes output bytes also follows `spec/VQS-1.md`
     section 12, with new conformance vectors.

   Merge after CI and the Release dry run pass.
2. **Tag the merge commit and push the tag:**
   `git tag -a vX.Y.Z -m "vikshep-quantum X.Y.Z"`, then
   `git push origin vX.Y.Z`.
3. **Approve the `pypi` and `crates-io` deployments** if reviewers are set.
   The crates job publishes one crate at a time with
   `scripts/publish-crates.sh`, waiting out the crates.io rate limit. If it
   fails anyway, re-run the job: crates already published are skipped.
4. **Check from a clean environment.**
   - This prints `0`:
     `python -m venv /tmp/v && /tmp/v/bin/pip install vikshep-quantum==X.Y.Z && /tmp/v/bin/python -c "import vikshep_quantum as vq; print(vq.conformance_selftest()['failed'])"`
   - In a new cargo project, this builds:
     `cargo add vikshep-statevec@=X.Y.Z vikshep-qcore@=X.Y.Z && cargo build`

PyPI and crates.io versions are permanent: a broken release is yanked and
fixed by a new version, never replaced.
