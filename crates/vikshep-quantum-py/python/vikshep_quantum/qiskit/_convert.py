"""Conversion of Qiskit circuits into VQS-1 circuits.

A Qiskit gate whose matrix is a VQS-1 gate (section 4 of the specification),
optionally with controls, becomes one VQS-1 operation with the same
parameters. A ``UnitaryGate`` becomes an explicit matrix. Every other gate
is replaced by its Qiskit definition, recursively. Definitions pass gate
angles through with at most exact arithmetic (sums, halves), so this
conversion computes no sine, cosine or matrix product of its own.

Measurements must come at the end: once a qubit is measured, no gate may act
on it again. A reset is accepted only on a qubit that nothing has touched
yet, where it does nothing. Global phases are dropped: they change no
probability, sample or expectation value.
"""

from __future__ import annotations

import math
from dataclasses import dataclass

import numpy as np
from qiskit.circuit import ControlFlowOp, ControlledGate, QuantumCircuit
from qiskit.exceptions import QiskitError

import vikshep_quantum as vq


def _fixed(name):
    return lambda params: (name, params)


# Qiskit gate name -> function of the float parameters giving
# (VQS-1 gate name, VQS-1 parameters). Each entry has the same matrix,
# global phase included, as the Qiskit gate.
_NATIVE = {
    "id": _fixed("id"),
    "x": _fixed("x"),
    "y": _fixed("y"),
    "z": _fixed("z"),
    "h": _fixed("h"),
    "s": _fixed("s"),
    "sdg": _fixed("sdg"),
    "t": _fixed("t"),
    "tdg": _fixed("tdg"),
    "sx": _fixed("sx"),
    "rx": _fixed("rx"),
    "ry": _fixed("ry"),
    "rz": _fixed("rz"),
    "p": _fixed("p"),
    "u": _fixed("u"),
    "swap": _fixed("swap"),
    "u1": _fixed("p"),
    "u3": _fixed("u"),
    "u2": lambda params: ("u", [math.pi / 2, params[0], params[1]]),
}

# Number of parameters of each gate in ``_NATIVE``. A gate object with a
# different count (the base gate of a controlled ``CUGate`` carries four) is
# not that gate and goes through its definition.
_ARITY = {name: 3 if name in ("u", "u3") else 2 if name == "u2" else 1 for name in _NATIVE}
for _name in ("id", "x", "y", "z", "h", "s", "sdg", "t", "tdg", "sx", "swap"):
    _ARITY[_name] = 0

_SKIPPED = {"barrier", "delay", "global_phase"}


@dataclass(frozen=True)
class Converted:
    """A converted circuit and its final measurements."""

    circuit: vq.Circuit
    """The VQS-1 circuit, without the measurements."""

    measured: dict[int, int]
    """Classical bit index -> index of the qubit last measured into it."""

    num_clbits: int
    """Number of classical bits of the Qiskit circuit."""


def from_qiskit(circuit: QuantumCircuit) -> vq.Circuit:
    """Convert a Qiskit circuit into a :class:`vikshep_quantum.Circuit`.

    Final measurements are dropped. Qubit ``i`` of the result is
    ``circuit.qubits[i]``. Raises ``QiskitError`` for a mid-circuit
    measurement, a reset after a gate, classical control flow, an unbound
    parameter or an operation that has no definition.
    """
    return convert(circuit).circuit


def convert(circuit: QuantumCircuit) -> Converted:
    """Convert a Qiskit circuit and record its final measurements."""
    if circuit.num_qubits == 0:
        raise QiskitError("Vikshep Quantum needs a circuit with at least one qubit")
    if circuit.num_qubits > vq.MAX_QUBITS:
        raise QiskitError(
            f"circuit has {circuit.num_qubits} qubits; Vikshep Quantum supports up to {vq.MAX_QUBITS}"
        )
    walker = _Walker(vq.Circuit(circuit.num_qubits))
    walker.walk(circuit, list(range(circuit.num_qubits)), list(range(circuit.num_clbits)))
    return Converted(walker.circuit, dict(walker.measured), circuit.num_clbits)


def _floats(op) -> list[float]:
    try:
        return [float(p) for p in op.params]
    except TypeError as err:
        raise QiskitError(
            f"gate {op.name!r} has an unbound parameter; assign parameter values first"
        ) from err


def _native(op):
    """(VQS-1 name or "unitary", parameters or matrix), or None."""
    if op.name == "unitary":
        return "unitary", np.asarray(op.params[0])
    rule = _NATIVE.get(op.name)
    if rule is None or len(op.params) != _ARITY[op.name]:
        return None
    return rule(_floats(op))


class _Walker:
    def __init__(self, circuit: vq.Circuit):
        self.circuit = circuit
        self.touched: set[int] = set()
        self.measured_qubits: set[int] = set()
        self.measured: dict[int, int] = {}

    def walk(self, circuit: QuantumCircuit, qubits: list[int], clbits: list[int]) -> None:
        qmap = dict(zip(circuit.qubits, qubits, strict=True))
        cmap = dict(zip(circuit.clbits, clbits, strict=True))
        for inst in circuit.data:
            op = inst.operation
            qs = [qmap[q] for q in inst.qubits]
            name = op.name
            if name in _SKIPPED:
                continue
            if name == "measure":
                self.measured_qubits.add(qs[0])
                self.measured[cmap[inst.clbits[0]]] = qs[0]
                continue
            if name == "reset":
                if qs[0] in self.touched or qs[0] in self.measured_qubits:
                    raise QiskitError(
                        f"reset of qubit {qs[0]} after it was used is not supported "
                        "(mid-circuit measurement and reset are not part of VQS-1 yet)"
                    )
                continue
            if isinstance(op, ControlFlowOp):
                raise QiskitError(f"classical control flow ({name}) is not supported")
            hit = self.measured_qubits.intersection(qs)
            if hit:
                raise QiskitError(
                    f"{name!r} acts on qubit {min(hit)} after it was measured; "
                    "measurements must come at the end of the circuit"
                )
            if self._append_native(op, qs):
                self.touched.update(qs)
            else:
                # Qubits count as touched only once a VQS-1 operation acts on
                # them, so a definition that starts with resets (``initialize``)
                # is accepted at the start of a circuit.
                definition = getattr(op, "definition", None)
                if definition is None:
                    raise QiskitError(
                        f"operation {name!r} has no definition and is not a VQS-1 gate"
                    )
                self.walk(definition, qs, [cmap[c] for c in inst.clbits])

    def _append_native(self, op, qs: list[int]) -> bool:
        if isinstance(op, ControlledGate):
            # ``CUGate`` has a fourth parameter, a phase on the controlled
            # subspace, that its base gate does not have; gates like it go
            # through their definitions.
            if len(op.params) != len(op.base_gate.params):
                return False
            spec = _native(op.base_gate)
            if spec is None:
                return False
            n_ctrl = op.num_ctrl_qubits
            controls, targets = qs[:n_ctrl], qs[n_ctrl:]
            # Open controls (a 0 bit in ctrl_state; bit k is control k) are
            # conjugated with X, which only moves amplitudes.
            open_controls = [c for k, c in enumerate(controls) if not (op.ctrl_state >> k) & 1]
            for c in open_controls:
                self.circuit.x(c)
            self._push(spec, targets, controls)
            for c in open_controls:
                self.circuit.x(c)
            return True
        spec = _native(op)
        if spec is None:
            return False
        self._push(spec, qs, [])
        return True

    def _push(self, spec, targets: list[int], controls: list[int]) -> None:
        name, arg = spec
        try:
            if name == "unitary":
                self.circuit.unitary(arg, targets, controls)
            else:
                self.circuit.gate(name, targets, arg, controls)
        except ValueError as err:
            raise QiskitError(str(err)) from err
