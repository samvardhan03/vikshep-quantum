#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Do quantum simulators give the same bits when the machine changes?

Runs fixed circuits with fixed seeds on Qiskit Aer, PennyLane Lightning and
Vikshep Quantum under several settings (thread count, precision, gate fusion,
kernel) and records, for each setting, hashes of the state, the
probabilities and one million measurement shots, the expectation values of a
few observables, and the trajectory of a shot-based QAOA optimisation.

    python divergence.py run --label NAME --out DIR [--sims aer,lightning,vikshep] [--quick]
    python divergence.py report DIR [DIR ...]

`run` writes DIR/summary.json and compares every setting with the default
setting of the same simulator and precision on this machine. `report` prints
a Markdown report; given summaries from several machines it also compares
the machines with each other.

Every setting runs in its own process, so that OMP_NUM_THREADS takes effect.
"""

import argparse
import hashlib
import json
import math
import os
import platform
import subprocess
import sys
import tempfile
import time

import numpy as np

SCHEMA = "vikshep.divergence/1"
SEED = 2026
SHOTS = 1_000_000
TRAIN_SHOTS = 10_000
TRAIN_ITERS = 150

# ---------------------------------------------------------------- circuits


def random_circuit(n, layers, rng):
    """Layers of random single-qubit rotations and a brick of CX or CZ."""
    ops = []
    for layer in range(layers):
        for q in range(n):
            g = ("rx", "ry", "rz")[int(rng.integers(3))]
            ops.append((g, float(rng.uniform(0.0, 2.0 * math.pi)), q))
        for q in range(layer % 2, n - 1, 2):
            ops.append(("cx" if int(rng.integers(2)) == 0 else "cz", q, q + 1))
    return ops


def qft_circuit(n, rng):
    """A random product state followed by the quantum Fourier transform
    (without the final swaps)."""
    ops = [("ry", float(rng.uniform(0.0, math.pi)), q) for q in range(n)]
    for j in reversed(range(n)):
        ops.append(("h", j))
        for k in reversed(range(j)):
            ops.append(("cp", math.pi / 2 ** (j - k), k, j))
    return ops


def circuits(quick):
    if quick:
        return {"random12": (12, random_circuit(12, 10, np.random.default_rng([SEED, 0])))}
    return {
        "random16": (16, random_circuit(16, 30, np.random.default_rng([SEED, 1]))),
        "random20": (20, random_circuit(20, 16, np.random.default_rng([SEED, 2]))),
        "qft20": (20, qft_circuit(20, np.random.default_rng([SEED, 3]))),
    }


def observables(n):
    """name -> list of (coefficient, [(pauli, qubit), ...])."""
    return {
        "Z0Z1": [(1.0, [("Z", 0), ("Z", 1)])],
        "X3Y7Z11": [(1.0, [("X", 3), ("Y", 7), ("Z", 11)])],
        "Zall": [(1.0, [("Z", q) for q in range(n)])],
        "ising": [(1.0, [("Z", q), ("Z", q + 1)]) for q in range(n - 1)]
        + [(0.5, [("X", q)]) for q in range(n)],
    }


# QAOA for MaxCut on a 3-regular graph of 16 vertices (a ring plus its
# diameters), depth p = 2, optimised by SPSA on sampled energies.
TRAIN_N = 16
EDGES = [(i, (i + 1) % TRAIN_N) for i in range(TRAIN_N)] + [
    (i, i + TRAIN_N // 2) for i in range(TRAIN_N // 2)
]
TRAIN_INIT = [0.30, 0.60, 0.50, 0.30]


def qaoa_ops(params):
    ops = [("h", q) for q in range(TRAIN_N)]
    for layer in range(len(params) // 2):
        gamma, beta = params[2 * layer], params[2 * layer + 1]
        for i, j in EDGES:
            ops += [("cx", i, j), ("rz", 2.0 * gamma, j), ("cx", i, j)]
        for q in range(TRAIN_N):
            ops.append(("rx", 2.0 * beta, q))
    return ops


def cut_table():
    idx = np.arange(2**TRAIN_N, dtype=np.int64)
    bits = (idx[:, None] >> np.arange(TRAIN_N)) & 1
    return sum((bits[:, i] != bits[:, j]).astype(np.int64) for i, j in EDGES)


# ---------------------------------------------------------------- adapters
#
# Qubit 0 is the least significant bit of a basis-state index, as in Qiskit
# and Vikshep Quantum. PennyLane's wire 0 is the most significant bit, so
# qubit q is placed on wire n-1-q; then all three simulators index states,
# probabilities and samples the same way.


def to_qiskit(n, ops):
    from qiskit import QuantumCircuit

    qc = QuantumCircuit(n)
    for op in ops:
        g = op[0]
        if g == "h":
            qc.h(op[1])
        elif g in ("rx", "ry", "rz", "cx", "cz"):
            getattr(qc, g)(op[1], op[2])
        elif g == "cp":
            qc.cp(op[1], op[2], op[3])
        else:
            raise ValueError(g)
    return qc


def to_vikshep(n, ops):
    import vikshep_quantum as vq

    c = vq.Circuit(n)
    for op in ops:
        g = op[0]
        if g == "h":
            c.h(op[1])
        elif g in ("rx", "ry", "rz", "cx", "cz"):
            getattr(c, g)(op[1], op[2])
        elif g == "cp":
            c.cp(op[1], op[2], op[3])
        else:
            raise ValueError(g)
    return c


def pennylane_body(n, ops):
    import pennylane as qml

    def w(q):
        return n - 1 - q

    for op in ops:
        g = op[0]
        if g == "h":
            qml.Hadamard(w(op[1]))
        elif g == "rx":
            qml.RX(op[1], w(op[2]))
        elif g == "ry":
            qml.RY(op[1], w(op[2]))
        elif g == "rz":
            qml.RZ(op[1], w(op[2]))
        elif g == "cx":
            qml.CNOT([w(op[1]), w(op[2])])
        elif g == "cz":
            qml.CZ([w(op[1]), w(op[2])])
        elif g == "cp":
            qml.ControlledPhaseShift(op[1], [w(op[2]), w(op[3])])
        else:
            raise ValueError(g)


def pennylane_observable(n, terms):
    import pennylane as qml

    paulis = {"X": qml.PauliX, "Y": qml.PauliY, "Z": qml.PauliZ}

    def product(term):
        ops = [paulis[p](n - 1 - q) for p, q in term]
        return ops[0] if len(ops) == 1 else qml.prod(*ops)

    if len(terms) == 1 and terms[0][0] == 1.0:
        return product(terms[0][1])
    return qml.dot([c for c, _ in terms], [product(t) for _, t in terms])


def qiskit_observable(n, terms):
    from qiskit.quantum_info import SparsePauliOp

    return SparsePauliOp.from_sparse_list(
        [("".join(p for p, _ in t), [q for _, q in t], c) for c, t in terms], num_qubits=n
    )


def pauli_string(term):
    return " ".join(f"{p}{q}" for p, q in term)


def bits_to_index(bits):
    n = bits.shape[1]
    weights = np.left_shift(np.uint64(1), np.arange(n - 1, -1, -1, dtype=np.uint64))
    return (bits.astype(np.uint64) * weights).sum(axis=1, dtype=np.uint64)


# Each backend returns (state, probabilities, {observable: value}, samples)
# for a circuit, and a sampler for training: sampler(ops, seed) -> indices.


class Aer:
    def __init__(self, cfg):
        from qiskit_aer import AerSimulator

        self.sim = AerSimulator(
            method="statevector",
            precision=cfg["precision"],
            max_parallel_threads=cfg["threads"],
            fusion_enable=cfg["fusion"],
        )

    def evaluate(self, n, ops, shots):
        qc = to_qiskit(n, ops)
        qa = qc.copy()
        qa.save_statevector()
        qa.save_probabilities()
        obs = observables(n)
        for name, terms in obs.items():
            qa.save_expectation_value(qiskit_observable(n, terms), list(range(n)), label=name)
        data = self.sim.run(qa, shots=1).result().data(0)
        state = np.asarray(data["statevector"])
        probs = np.asarray(data["probabilities"])
        exps = {name: float(data[name]) for name in obs}
        return state, probs, exps, self.sample(n, ops, shots, SEED)

    def sample(self, n, ops, shots, seed):
        qm = to_qiskit(n, ops)
        qm.measure_all()
        mem = self.sim.run(qm, shots=shots, seed_simulator=seed, memory=True).result().get_memory(0)
        return np.fromiter((int(s, 2) for s in mem), dtype=np.uint64, count=len(mem))


class Lightning:
    def __init__(self, cfg):
        self.kw = {"c_dtype": np.complex128 if cfg["precision"] == "double" else np.complex64}
        if cfg.get("kernel"):
            self.kw["kernel_name"] = cfg["kernel"]

    def evaluate(self, n, ops, shots):
        import pennylane as qml

        dev = qml.device("lightning.qubit", wires=n, **self.kw)
        obs = observables(n)

        @qml.qnode(dev)
        def f_state():
            pennylane_body(n, ops)
            return qml.state()

        @qml.qnode(dev)
        def f_meas():
            pennylane_body(n, ops)
            return (
                qml.probs(wires=list(range(n))),
                *[qml.expval(pennylane_observable(n, t)) for t in obs.values()],
            )

        state = np.asarray(f_state())
        res = f_meas()
        probs = np.asarray(res[0])
        exps = {name: float(v) for name, v in zip(obs, res[1:])}
        return state, probs, exps, self.sample(n, ops, shots, SEED)

    def sample(self, n, ops, shots, seed):
        import pennylane as qml

        dev = qml.device("lightning.qubit", wires=n, seed=seed, **self.kw)

        @qml.set_shots(shots)
        @qml.qnode(dev)
        def f():
            pennylane_body(n, ops)
            return qml.sample(wires=list(range(n)))

        return bits_to_index(np.asarray(f()))


class Vikshep:
    def __init__(self, cfg):
        self.threads = cfg["threads"]

    def _run(self, n, ops):
        import vikshep_quantum as vq

        sim = vq.Simulator(n, threads=self.threads)
        sim.run(to_vikshep(n, ops))
        return sim

    def evaluate(self, n, ops, shots):
        sim = self._run(n, ops)
        exps = {}
        for name, terms in observables(n).items():
            # Sums of terms are added here, in a fixed order, in binary64.
            exps[name] = sum(c * float(sim.expectation(pauli_string(t))) for c, t in terms)
        return sim.state(), sim.probabilities(), exps, sim.sample(shots, seed=SEED)

    def sample(self, n, ops, shots, seed):
        return self._run(n, ops).sample(shots, seed=seed)


BACKENDS = {"aer": Aer, "lightning": Lightning, "vikshep": Vikshep}

# ---------------------------------------------------------------- worker


def sha256(a):
    return hashlib.sha256(np.ascontiguousarray(a).tobytes()).hexdigest()


def spsa(sampler, cuts):
    """SPSA on sampled MaxCut energies. Gains use only division, which is
    correctly rounded, so the optimiser itself is the same on every machine;
    any difference comes from the simulator."""
    rng = np.random.default_rng([SEED, 99])
    theta = list(TRAIN_INIT)
    traj = []
    for k in range(TRAIN_ITERS):
        a_k = 0.2 / (k + 11.0)
        c_k = 0.15 / (1.0 + k / 50.0)
        delta = [1.0 if b else -1.0 for b in rng.integers(0, 2, size=len(theta))]
        f = []
        for sign, s in ((1.0, 2 * k), (-1.0, 2 * k + 1)):
            probe = [t + sign * c_k * d for t, d in zip(theta, delta)]
            samples = sampler(TRAIN_N, qaoa_ops(probe), TRAIN_SHOTS, SEED + 1000 + s)
            total = int(cuts[samples.astype(np.int64)].sum())
            f.append(-total / TRAIN_SHOTS)
        g = (f[0] - f[1]) / (2.0 * c_k)
        theta = [t - a_k * g * d for t, d in zip(theta, delta)]
        traj.append({"params": [x.hex() for x in theta], "f": [x.hex() for x in f]})
    return traj


def worker(spec_path):
    with open(spec_path) as fh:
        spec = json.load(fh)
    cfg, workdir, quick = spec["config"], spec["workdir"], spec["quick"]
    backend = BACKENDS[cfg["sim"]](cfg)
    shots = 20_000 if quick else SHOTS
    out = {"circuits": {}}
    t0 = time.perf_counter()
    for name, (n, ops) in circuits(quick).items():
        state, probs, exps, samples = backend.evaluate(n, ops, shots)
        samples = np.asarray(samples, dtype="<u8")
        stem = os.path.join(workdir, f"{cfg['id']}-{name}")
        np.save(stem + "-state.npy", state)
        np.save(stem + "-probs.npy", probs)
        np.save(stem + "-samples.npy", samples)
        out["circuits"][name] = {
            "n_qubits": n,
            "n_ops": len(ops),
            "state_dtype": str(state.dtype),
            "state_sha256": sha256(state),
            "probs_dtype": str(probs.dtype),
            "probs_sha256": sha256(probs),
            "samples_sha256": sha256(samples),
            "expectations": {k: float(v).hex() for k, v in exps.items()},
        }
    out["seconds_circuits"] = round(time.perf_counter() - t0, 2)
    if not spec["skip_train"]:
        t0 = time.perf_counter()
        traj = spsa(backend.sample, cut_table())
        out["train"] = {
            "trajectory": traj,
            "trajectory_sha256": hashlib.sha256(json.dumps(traj).encode()).hexdigest(),
        }
        out["seconds_train"] = round(time.perf_counter() - t0, 2)
    print(json.dumps(out))


# ---------------------------------------------------------------- driver


def machine_info():
    info = {
        "system": platform.system(),
        "release": platform.release(),
        "machine": platform.machine(),
        "python": platform.python_version(),
        "cpu_count": os.cpu_count(),
        "cpu": platform.processor(),
        "simd": [],
    }
    try:
        if info["system"] == "Linux":
            with open("/proc/cpuinfo") as fh:
                text = fh.read()
            for line in text.splitlines():
                if line.startswith("model name") or line.startswith("Model"):
                    info["cpu"] = line.split(":", 1)[1].strip()
                    break
            flags = next((ln for ln in text.splitlines() if ln.startswith(("flags", "Features"))), "")
            info["simd"] = sorted(
                f for f in flags.split(":", 1)[-1].split()
                if f in ("avx2", "fma", "avx512f", "asimd", "sve", "sve2")
            )
        elif info["system"] == "Darwin":
            info["cpu"] = subprocess.run(
                ["sysctl", "-n", "machdep.cpu.brand_string"], capture_output=True, text=True
            ).stdout.strip()
    except OSError:
        pass
    return info


def versions(sims):
    from importlib.metadata import PackageNotFoundError, version

    out = {"numpy": version("numpy")}
    pkgs = {"aer": ["qiskit", "qiskit-aer"], "lightning": ["pennylane", "pennylane-lightning"],
            "vikshep": ["vikshep-quantum"]}
    for s in sims:
        for p in pkgs[s]:
            try:
                out[p] = version(p)
            except PackageNotFoundError:
                out[p] = None
    return out


def available(sim):
    probe = {
        "aer": "import qiskit_aer",
        "lightning": "import pennylane_lightning.lightning_qubit_ops",
        "vikshep": "import vikshep_quantum",
    }[sim]
    r = subprocess.run([sys.executable, "-I", "-c", probe], capture_output=True)
    return r.returncode == 0


def lightning_kernels():
    """Kernels Lightning accepts on this machine, besides the default."""
    found = []
    for k in ("LM", "AVX2", "AVX512"):
        code = (
            "import pennylane as qml, numpy as np\n"
            f"dev = qml.device('lightning.qubit', wires=2, kernel_name={k!r})\n"
            "@qml.qnode(dev)\n"
            "def f():\n    qml.RX(0.3, 0); qml.CNOT([0, 1]); return qml.state()\n"
            "f()\n"
        )
        if subprocess.run([sys.executable, "-I", "-c", code], capture_output=True).returncode == 0:
            found.append(k)
    return found


def configs(sims, quick, defaults_only=False):
    """Settings to run. With defaults_only: all threads and one thread, with
    the simulators' defaults otherwise (for comparing machines)."""
    cpus = os.cpu_count() or 1
    threads = sorted({t for t in (1, 2, 4, cpus) if t <= cpus})
    if quick or defaults_only:
        threads = sorted({1, cpus})
    out = []
    for sim in sims:
        precisions = ["single"] if sim == "vikshep" else ["double", "single"]
        for prec in precisions:
            ref = None
            variants = []
            # The default setting (all threads, defaults otherwise) first.
            if sim == "aer":
                for fusion in (True,) if defaults_only else (True, False):
                    for t in reversed(threads):
                        variants.append({"threads": t, "fusion": fusion})
            elif sim == "lightning":
                for t in reversed(threads):
                    variants.append({"threads": t, "kernel": None})
                for k in [] if defaults_only else lightning_kernels():
                    variants.append({"threads": cpus, "kernel": k})
            else:
                for t in reversed(threads):
                    variants.append({"threads": t})
            for v in variants:
                parts = [sim, prec, f"t{v['threads']}"]
                if "fusion" in v:
                    parts.append("fusion" if v["fusion"] else "nofusion")
                if v.get("kernel"):
                    parts.append(v["kernel"])
                cid = "-".join(parts)
                cfg = {"sim": sim, "precision": prec, "id": cid, **v}
                if ref is None:
                    ref = cid
                cfg["ref"] = ref
                out.append(cfg)
    return out


def compare(ref_stem, cur_stem):
    rs, cs = np.load(ref_stem + "-state.npy"), np.load(cur_stem + "-state.npy")
    rp, cp = np.load(ref_stem + "-probs.npy"), np.load(cur_stem + "-probs.npy")
    ra, ca = np.load(ref_stem + "-samples.npy"), np.load(cur_stem + "-samples.npy")
    size = len(rp)
    moved = np.abs(np.bincount(ra.astype(np.int64), minlength=size)
                   - np.bincount(ca.astype(np.int64), minlength=size)).sum() // 2
    return {
        "state_identical": rs.dtype == cs.dtype and rs.tobytes() == cs.tobytes(),
        "state_max_abs_diff": float(np.max(np.abs(rs.astype(np.complex128) - cs.astype(np.complex128)))),
        "probs_identical": rp.dtype == cp.dtype and rp.tobytes() == cp.tobytes(),
        "probs_max_abs_diff": float(np.max(np.abs(rp.astype(np.float64) - cp.astype(np.float64)))),
        "shots_differing": int(np.count_nonzero(ra != ca)),
        "shots_moved_in_counts": int(moved),
    }


def compare_train(ref, cur):
    first = None
    max_diff = 0.0
    for i, (a, b) in enumerate(zip(ref, cur)):
        pa = [float.fromhex(x) for x in a["params"]]
        pb = [float.fromhex(x) for x in b["params"]]
        if first is None and (pa != pb or a["f"] != b["f"]):
            first = i
        max_diff = max(max_diff, max(abs(x - y) for x, y in zip(pa, pb)))
    return {"identical": first is None, "first_differing_iteration": first,
            "max_param_diff": max_diff}


def run(args):
    sims = [s for s in args.sims.split(",") if s]
    missing = [s for s in sims if not available(s)]
    if missing:
        print(f"skipping (not installed or not functional here): {', '.join(missing)}", file=sys.stderr)
    sims = [s for s in sims if s not in missing]
    os.makedirs(args.out, exist_ok=True)
    summary = {
        "schema": SCHEMA,
        "label": args.label,
        "machine": machine_info(),
        "versions": versions(sims),
        "settings": {"seed": SEED, "shots": 20_000 if args.quick else SHOTS,
                     "train_shots": TRAIN_SHOTS, "train_iters": TRAIN_ITERS, "quick": args.quick},
        "runs": [],
    }
    with tempfile.TemporaryDirectory(prefix="divergence-") as workdir:
        refs = {}
        for cfg in configs(sims, args.quick, args.defaults_only):
            spec = os.path.join(workdir, cfg["id"] + ".json")
            with open(spec, "w") as fh:
                json.dump({"config": cfg, "workdir": workdir, "quick": args.quick,
                           "skip_train": args.skip_train}, fh)
            env = dict(os.environ, OMP_NUM_THREADS=str(cfg["threads"]))
            print(f"[{args.label}] {cfg['id']} ...", file=sys.stderr, flush=True)
            # -X faulthandler prints a traceback if the process crashes in
            # native code (it ignores PYTHONFAULTHANDLER under -I).
            r = subprocess.run([sys.executable, "-I", "-X", "faulthandler", os.path.abspath(__file__),
                                "worker", spec], capture_output=True, text=True, env=env)
            if r.returncode != 0:
                # Record the failure and go on: a crash is a result too.
                tail = r.stderr.strip().splitlines()[-20:]
                print(f"worker failed: {cfg['id']} (exit code {r.returncode})", file=sys.stderr)
                print("\n".join(tail), file=sys.stderr)
                summary["runs"].append({"id": cfg["id"], "config": cfg,
                                        "error": {"exit_code": r.returncode, "stderr_tail": tail}})
                for name in circuits(args.quick):
                    for suffix in ("-state.npy", "-probs.npy", "-samples.npy"):
                        path = os.path.join(workdir, f"{cfg['id']}-{name}{suffix}")
                        if os.path.exists(path):
                            os.remove(path)
                continue
            res = json.loads(r.stdout.strip().splitlines()[-1])
            entry = {"id": cfg["id"], "config": cfg, **res}
            is_ref = cfg["ref"] == cfg["id"]
            if is_ref:
                refs[cfg["id"]] = entry
            ref = refs.get(cfg["ref"])  # None if the default setting failed
            for name, c in entry["circuits"].items():
                ref_stem = os.path.join(workdir, f"{cfg['ref']}-{name}")
                cur_stem = os.path.join(workdir, f"{cfg['id']}-{name}")
                if ref is not None:
                    c["vs_default"] = compare(ref_stem, cur_stem)
                    re_ = ref["circuits"][name]["expectations"]
                    c["vs_default"]["expectation_abs_diff"] = {
                        k: abs(float.fromhex(v) - float.fromhex(re_[k])) for k, v in c["expectations"].items()
                    }
                if not is_ref:
                    for suffix in ("-state.npy", "-probs.npy", "-samples.npy"):
                        os.remove(cur_stem + suffix)
            if "train" in entry and ref is not None:
                entry["train"]["vs_default"] = compare_train(ref["train"]["trajectory"],
                                                             entry["train"]["trajectory"])
            summary["runs"].append(entry)
            if args.save_arrays and is_ref:
                for name in entry["circuits"]:
                    stem = os.path.join(workdir, f"{cfg['id']}-{name}")
                    np.savez_compressed(os.path.join(args.out, f"{cfg['id']}-{name}.npz"),
                                        state=np.load(stem + "-state.npy"),
                                        probs=np.load(stem + "-probs.npy"),
                                        samples=np.load(stem + "-samples.npy"))
        # Agreement between simulators (default settings): a check that the
        # three adapters build the same circuits.
        agree = {}
        ref_ids = list(refs)
        for name in circuits(args.quick):
            probs = {rid: np.load(os.path.join(workdir, f"{rid}-{name}-probs.npy")).astype(np.float64)
                     for rid in ref_ids}
            base = ref_ids[0]
            agree[name] = {rid: float(np.max(np.abs(probs[rid] - probs[base]))) for rid in ref_ids[1:]}
            agree[name]["_base"] = base
        summary["agreement_probs_max_abs_diff"] = agree
    with open(os.path.join(args.out, "summary.json"), "w") as fh:
        json.dump(summary, fh, sort_keys=True, separators=(",", ":"))
        fh.write("\n")
    print(os.path.join(args.out, "summary.json"))


# ---------------------------------------------------------------- report


def fmt_diff(x):
    return "0" if x == 0 else f"{x:.1e}"


def norm_id(cid, cpus):
    """Name a machine's all-threads setting `tall`, so that default settings
    match across machines with different core counts."""
    parts = cid.split("-")
    if len(parts) > 2 and parts[2] == f"t{cpus}":
        parts[2] = "tall"
    return "-".join(parts)


def report(args):
    sums = []
    for d in args.dirs:
        with open(os.path.join(d, "summary.json")) as fh:
            sums.append(json.load(fh))
    lines = ["# Divergence results", ""]
    for s in sums:
        m = s["machine"]
        lines.append(f"- **{s['label']}**: {m['system']} {m['machine']}, {m['cpu']}, "
                     f"{m['cpu_count']} logical CPUs, SIMD {', '.join(m['simd']) or 'n/a'}; "
                     + ", ".join(f"{k} {v}" for k, v in sorted(s["versions"].items())))
    lines.append("")
    for s in sums:
        lines += [f"## {s['label']}: each setting against the default setting", "",
                  "Default setting: all threads, fusion on (Aer), default kernel (Lightning). "
                  "`=` means identical bytes; otherwise the largest absolute difference. "
                  f"Shots: of {s['settings']['shots']:,} with the same seed, how many differ.", ""]
        names = next(list(r["circuits"]) for r in s["runs"] if "circuits" in r)
        head = "| Setting | " + " | ".join(f"{n} state | {n} expect. | {n} shots" for n in names) + " | training |"
        lines += [head, "|" + "---|" * (1 + 3 * len(names) + 1)]
        for r in s["runs"]:
            if "error" in r or any("vs_default" not in r["circuits"][n] for n in names):
                note = (f"failed (exit code {r['error']['exit_code']})" if "error" in r
                        else "not compared: the default setting failed")
                lines.append(f"| `{r['id']}` | {note} | " + " | ".join([""] * (3 * len(names))) + " |")
                continue
            if r["id"] == r["config"]["ref"]:
                cells = []
                for n in names:
                    cells += ["(default)", "", ""]
                lines.append(f"| `{r['id']}` | " + " | ".join(cells) + " | (default) |")
                continue
            cells = []
            for n in names:
                v = r["circuits"][n]["vs_default"]
                cells.append("=" if v["state_identical"] else fmt_diff(v["state_max_abs_diff"]))
                e = max(v["expectation_abs_diff"].values())
                cells.append("=" if e == 0 else fmt_diff(e))
                cells.append(str(v["shots_differing"]))
            tr = r.get("train", {}).get("vs_default")
            if tr is None:
                tcell = "n/a"
            elif tr["identical"]:
                tcell = "identical"
            else:
                tcell = f"differs from step {tr['first_differing_iteration']} (max param diff {tr['max_param_diff']:.1e})"
            lines.append(f"| `{r['id']}` | " + " | ".join(cells) + f" | {tcell} |")
        lines.append("")
        agree = s.get("agreement_probs_max_abs_diff", {})
        if agree:
            lines.append("Agreement between simulators (largest probability difference against "
                         f"`{next(iter(agree.values()))['_base']}`): "
                         + "; ".join(f"{n}: " + ", ".join(f"{k} {v:.1e}" for k, v in a.items() if k != "_base")
                                     for n, a in agree.items()))
            lines.append("")
    if len(sums) > 1:
        lines += ["## Across machines", "",
                  "Settings present on every machine (`tall` is each machine's own thread count, "
                  "the default). For each, are the bytes the same on every machine? "
                  "\"differs\" lists the circuits that differ.", "",
                  "| Setting | state | probabilities | shots | expectations (max diff) | training |",
                  "|---|---|---|---|---|---|"]
        by = [{norm_id(r["id"], s["machine"]["cpu_count"]): r for r in s["runs"]} for s in sums]
        common = [i for i in by[0] if all(i in b for b in by[1:])]
        for cid in common:
            rs = [b[cid] for b in by]
            failed = [s["label"] for s, r in zip(sums, rs) if "error" in r]
            if failed:
                lines.append(f"| `{cid}` | failed on {', '.join(failed)} | | | | |")
                continue
            names = list(rs[0]["circuits"])
            cells = []
            for key in ("state_sha256", "probs_sha256", "samples_sha256"):
                bad = [n for n in names if any(r["circuits"][n][key] != rs[0]["circuits"][n][key] for r in rs)]
                cells.append("same" if not bad else "differs (" + ", ".join(bad) + ")")
            e = max(abs(float.fromhex(r["circuits"][n]["expectations"][k])
                        - float.fromhex(rs[0]["circuits"][n]["expectations"][k]))
                    for r in rs for n in names for k in rs[0]["circuits"][n]["expectations"])
            cells.append(fmt_diff(e))
            tr = "n/a"
            if all("train" in r for r in rs):
                t = [compare_train(rs[0]["train"]["trajectory"], r["train"]["trajectory"]) for r in rs[1:]]
                if all(x["identical"] for x in t):
                    tr = "identical"
                else:
                    first = min(x["first_differing_iteration"] for x in t if not x["identical"])
                    tr = f"differs from step {first}"
            cells.append(tr)
            lines.append(f"| `{cid}` | " + " | ".join(cells) + " |")
        lines.append("")
    print("\n".join(lines))


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = p.add_subparsers(dest="cmd", required=True)
    r = sub.add_parser("run")
    r.add_argument("--label", required=True)
    r.add_argument("--out", required=True)
    r.add_argument("--sims", default="aer,lightning,vikshep")
    r.add_argument("--quick", action="store_true", help="one small circuit, few shots (a smoke test)")
    r.add_argument("--skip-train", action="store_true")
    r.add_argument("--defaults-only", action="store_true",
                   help="only the default settings with all threads and with one thread")
    r.add_argument("--save-arrays", action="store_true",
                   help="keep the default settings' states, probabilities and samples as .npz")
    w = sub.add_parser("worker")
    w.add_argument("spec")
    rep = sub.add_parser("report")
    rep.add_argument("dirs", nargs="+")
    args = p.parse_args()
    if args.cmd == "run":
        run(args)
    elif args.cmd == "worker":
        worker(args.spec)
    else:
        report(args)


if __name__ == "__main__":
    main()
