"""The wheel runs the whole conformance suite, and states built through the
Python API have the bytes the committed vectors record (VQS-1 suite v1)."""

import hashlib
import json
import math
from pathlib import Path

import vikshep_quantum as vq

ROOT = Path(__file__).resolve().parents[3]
EXPECTED = json.loads((ROOT / "conformance" / "vectors" / "v1" / "expected.json").read_text())


def expected(case, output):
    return next(o for o in EXPECTED["cases"][case] if o["name"] == output)["sha3_256"]


def sha3(data: bytes) -> str:
    return hashlib.sha3_256(data).hexdigest()


def check_circuit_case(case, circuit):
    sim = vq.Simulator(circuit.n_qubits)
    sim.run(circuit)
    # complex64 little-endian is the canonical layout: re, im as binary32.
    assert sha3(sim.state().astype("<c8").tobytes()) == expected(case, "state")
    assert sim.state_sha3() == expected(case, "state")
    assert sha3(circuit.canonical_json().encode()) == expected(case, "circuit")
    manifest = json.loads(sim.manifest())
    assert sha3(manifest["manifest_hash"].encode()) == expected(case, "manifest_hash")


def test_versions_match_the_vectors():
    assert EXPECTED["suite"] == "vqs1-v1"
    assert EXPECTED["vqs_version"] == vq.VQS_VERSION == 1
    assert EXPECTED["numerics_version"] == vq.NUMERICS_VERSION == 3


def test_selftest_passes_every_case():
    result = vq.conformance_selftest()
    n = len(EXPECTED["cases"])
    assert (result["cases"], result["passed"], result["failed"]) == (n, n, 0)
    report = json.loads(result["report"])
    assert report["summary"] == {"cases": n, "fail": 0, "pass": n}
    assert report["backend"] == "cpu"


def test_bell_built_in_python_matches_the_vector():
    check_circuit_case("circuit/bell", vq.Circuit(2).h(0).cx(0, 1))


def test_ghz_built_in_python_matches_the_vector():
    c = vq.Circuit(10).h(0)
    for q in range(1, 10):
        c.cx(q - 1, q)
    check_circuit_case("circuit/ghz/n10", c)


def test_qft_built_in_python_matches_the_vector():
    n, x = 8, 77
    c = vq.Circuit(n)
    for q in range(n):
        if (x >> q) & 1:
            c.x(q)
    for j in reversed(range(n)):
        c.h(j)
        for k in reversed(range(j)):
            c.cp(math.pi / (1 << (j - k)), k, j)
    for q in range(n // 2):
        c.swap(q, n - 1 - q)
    check_circuit_case("circuit/qft/n8/x77", c)
