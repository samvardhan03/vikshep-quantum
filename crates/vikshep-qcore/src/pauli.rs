//! Pauli strings (`spec/VQS-1.md` section 9).
//!
//! A Pauli string assigns one of I, X, Y, Z to every qubit. Its text form
//! names only the non-identity factors with their qubit index, separated by
//! spaces, for example `X0 Z2 Y5`; the identity on every qubit is `I`. This
//! avoids the left-to-right or right-to-left ambiguity of dense strings.

use crate::circuit::validate_n_qubits;
use crate::error::QError;

/// A single-qubit Pauli operator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pauli {
    /// Identity.
    I,
    /// Pauli X.
    X,
    /// Pauli Y.
    Y,
    /// Pauli Z.
    Z,
}

/// A tensor product of single-qubit Pauli operators.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PauliString {
    n_qubits: u32,
    /// Bit `q` set when qubit `q` carries X or Y.
    x_mask: u64,
    /// Bit `q` set when qubit `q` carries Z or Y.
    z_mask: u64,
}

impl PauliString {
    /// Build from `(qubit, operator)` factors; unnamed qubits carry I.
    pub fn new(n_qubits: u32, factors: &[(u32, Pauli)]) -> Result<Self, QError> {
        validate_n_qubits(n_qubits)?;
        let (mut x_mask, mut z_mask, mut seen) = (0u64, 0u64, 0u64);
        for &(q, p) in factors {
            if q >= n_qubits {
                return Err(QError::QubitOutOfRange { qubit: q, n_qubits });
            }
            let bit = 1u64 << q;
            if seen & bit != 0 {
                return Err(QError::DuplicateQubit(q));
            }
            seen |= bit;
            match p {
                Pauli::I => {}
                Pauli::X => x_mask |= bit,
                Pauli::Z => z_mask |= bit,
                Pauli::Y => {
                    x_mask |= bit;
                    z_mask |= bit;
                }
            }
        }
        Ok(Self {
            n_qubits,
            x_mask,
            z_mask,
        })
    }

    /// Parse the text form, for example `X0 Z2 Y5` (letters in either case).
    /// The empty string and `I` are the identity.
    pub fn parse(n_qubits: u32, text: &str) -> Result<Self, QError> {
        let mut factors = Vec::new();
        for token in text.split_whitespace() {
            let mut chars = token.chars();
            let letter = chars
                .next()
                .expect("split_whitespace yields non-empty tokens");
            let p = match letter.to_ascii_uppercase() {
                'I' => Pauli::I,
                'X' => Pauli::X,
                'Y' => Pauli::Y,
                'Z' => Pauli::Z,
                _ => {
                    return Err(QError::PauliSyntax(format!(
                        "unknown operator in {token:?} (expected I, X, Y or Z followed by a qubit index)"
                    )));
                }
            };
            let digits = chars.as_str();
            if digits.is_empty() {
                if p == Pauli::I {
                    continue; // a bare "I" is the identity on every qubit
                }
                return Err(QError::PauliSyntax(format!("{token:?} has no qubit index")));
            }
            let q: u32 = digits.parse().map_err(|_| {
                QError::PauliSyntax(format!("{token:?} has an invalid qubit index"))
            })?;
            factors.push((q, p));
        }
        Self::new(n_qubits, &factors)
    }

    /// Number of qubits.
    #[must_use]
    pub fn n_qubits(&self) -> u32 {
        self.n_qubits
    }

    /// Bit `q` set when qubit `q` carries X or Y.
    #[must_use]
    pub fn x_mask(&self) -> u64 {
        self.x_mask
    }

    /// Bit `q` set when qubit `q` carries Z or Y.
    #[must_use]
    pub fn z_mask(&self) -> u64 {
        self.z_mask
    }

    /// Number of Y factors.
    #[must_use]
    pub fn y_count(&self) -> u32 {
        (self.x_mask & self.z_mask).count_ones()
    }

    /// The operator on qubit `q`.
    #[must_use]
    pub fn factor(&self, q: u32) -> Pauli {
        match ((self.x_mask >> q) & 1, (self.z_mask >> q) & 1) {
            (0, 0) => Pauli::I,
            (1, 0) => Pauli::X,
            (0, 1) => Pauli::Z,
            _ => Pauli::Y,
        }
    }

    /// Canonical text form: non-identity factors by ascending qubit, or `I`.
    #[must_use]
    pub fn canonical(&self) -> String {
        let parts: Vec<String> = (0..self.n_qubits)
            .filter_map(|q| match self.factor(q) {
                Pauli::I => None,
                Pauli::X => Some(format!("X{q}")),
                Pauli::Y => Some(format!("Y{q}")),
                Pauli::Z => Some(format!("Z{q}")),
            })
            .collect();
        if parts.is_empty() {
            "I".into()
        } else {
            parts.join(" ")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_canonical_round_trip() {
        let p = PauliString::parse(6, "z2 X0  Y5").unwrap();
        assert_eq!(p.canonical(), "X0 Z2 Y5");
        assert_eq!(p.x_mask(), 0b10_0001);
        assert_eq!(p.z_mask(), 0b10_0100);
        assert_eq!(p.y_count(), 1);
        assert_eq!(PauliString::parse(3, "").unwrap().canonical(), "I");
        assert_eq!(PauliString::parse(3, "I").unwrap().canonical(), "I");
        assert_eq!(PauliString::parse(3, "I1 Z0").unwrap().canonical(), "Z0");
    }

    #[test]
    fn bad_strings_are_rejected() {
        assert!(matches!(
            PauliString::parse(3, "Q0"),
            Err(QError::PauliSyntax(_))
        ));
        assert!(matches!(
            PauliString::parse(3, "X"),
            Err(QError::PauliSyntax(_))
        ));
        assert!(matches!(
            PauliString::parse(3, "Xa"),
            Err(QError::PauliSyntax(_))
        ));
        assert_eq!(
            PauliString::parse(3, "X3").unwrap_err(),
            QError::QubitOutOfRange {
                qubit: 3,
                n_qubits: 3
            }
        );
        assert_eq!(
            PauliString::parse(3, "X1 Z1").unwrap_err(),
            QError::DuplicateQubit(1)
        );
    }
}
