# Licensing

Copyright (C) 2026 Samvardhan Singh.

This repository uses two licences, chosen by path.

| Path | Licence | Text |
|---|---|---|
| `spec/` | Creative Commons Attribution 4.0 International (CC-BY-4.0) | [`LICENSES/CC-BY-4.0.txt`](LICENSES/CC-BY-4.0.txt) |
| `conformance/vectors/` | Creative Commons Attribution 4.0 International (CC-BY-4.0) | [`LICENSES/CC-BY-4.0.txt`](LICENSES/CC-BY-4.0.txt) |
| Everything else (source code, build files, scripts, tests, CI configuration, documentation) | GNU Affero General Public License v3.0 or later (AGPL-3.0-or-later) | [`LICENSE`](LICENSE) |

The specification and conformance vectors are under CC-BY-4.0 so that anyone
can implement and test a backend against VQS-1 independently, with
attribution.

## Commercial licence

The code is also available under a commercial licence from the copyright
holder, for use where the terms of the AGPL are not suitable. Contact:
shekhawatsamvardhan@gmail.com.

## Third-party components

Dependencies keep their own licences. Current direct dependencies:

| Crate | Licence | Use |
|---|---|---|
| `vikshep-numerics` 0.1.0, `vikshep-detmath` 0.1.0 | AGPL-3.0-or-later (same copyright holder) | flush-to-zero arithmetic, Philox streams, SHA3 OIDs, canonical JSON, portable sine and cosine (VDS-1) |
| `serde_json` 1.0.151 | MIT OR Apache-2.0 | reading the expected conformance vectors |
| `pyo3` 0.29.3 | MIT OR Apache-2.0 | Python bindings (`vikshep-quantum-py`, feature `python`) |
| `numpy` 0.29.0 (Rust crate) | BSD-2-Clause | numpy arrays in the Python bindings |

Through these, the build also uses `libm` 0.2.16 (MIT), `sha3` 0.12.0 (MIT
OR Apache-2.0) and `num-complex` (MIT OR Apache-2.0). The Python package
`vikshep-quantum` depends at run time on numpy (BSD-3-Clause), installed
separately and not bundled.

Test-only references, not distributed with any crate: pytest (MIT) runs the
Python tests; Qiskit (Apache-2.0), when installed, is the reference of
`crates/vikshep-quantum-py/tests/test_qiskit.py`. The comparison in
`demo/divergence` installs Qiskit and Qiskit Aer (Apache-2.0), and
PennyLane and PennyLane Lightning (Apache-2.0), from
`demo/divergence/requirements.txt`. No third-party code is included. The gate definitions follow the OpenQASM 3 specification.

## Contributions

Outside contributions require a Contributor License Agreement, so that the
code can continue to be offered under both licences.
