//! Runner, expected vectors and report (`spec/VQS-1.md` section 11.3).

use std::collections::BTreeMap;

use vikshep_numerics::jcs::{Value, object};
use vikshep_numerics::oid::{hex, sha3_256};
use vikshep_qcore::{NUMERICS_VERSION, StateBackend, VQS_VERSION};

use crate::SUITE;
use crate::cases::{Case, suite};

/// The committed expected vectors of suite v1, compiled into the library so
/// the suite runs without files (Python `conformance_selftest`).
const EMBEDDED: &str = include_str!("../../../conformance/vectors/v1/expected.json");

/// Length and hash of one output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Digest {
    /// Output name.
    pub name: String,
    /// `float32`, `uint64` or `utf8`.
    pub dtype: String,
    /// Number of bytes.
    pub bytes: usize,
    /// Lowercase hex SHA3-256 of the bytes.
    pub sha3_256: String,
}

impl Digest {
    fn value(&self) -> Value {
        object([
            ("bytes", Value::Int(self.bytes as i64)),
            ("dtype", Value::Str(self.dtype.clone())),
            ("name", Value::Str(self.name.clone())),
            ("sha3_256", Value::Str(self.sha3_256.clone())),
        ])
    }
}

/// Expected digests per case identifier.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Expected {
    cases: BTreeMap<String, Vec<Digest>>,
}

impl Expected {
    /// The vectors compiled into this library.
    pub fn embedded() -> Result<Self, String> {
        Self::parse(EMBEDDED)
    }

    /// Parse an `expected.json` document.
    pub fn parse(text: &str) -> Result<Self, String> {
        let doc: serde_json::Value =
            serde_json::from_str(text).map_err(|e| format!("expected vectors: {e}"))?;
        let suite_name = doc["suite"].as_str().unwrap_or_default();
        if suite_name != SUITE {
            return Err(format!(
                "expected vectors are for suite {suite_name:?}, this is {SUITE:?}"
            ));
        }
        let mut cases = BTreeMap::new();
        let listed = doc["cases"]
            .as_object()
            .ok_or("expected vectors: 'cases' is not an object")?;
        for (id, outputs) in listed {
            let outputs = outputs
                .as_array()
                .ok_or_else(|| format!("expected vectors: case {id:?} is not an array"))?;
            let mut digests = Vec::with_capacity(outputs.len());
            for o in outputs {
                let field = |k: &str| {
                    o[k].as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| format!("expected vectors: case {id:?} lacks {k:?}"))
                };
                digests.push(Digest {
                    name: field("name")?,
                    dtype: field("dtype")?,
                    bytes: o["bytes"]
                        .as_u64()
                        .ok_or_else(|| format!("expected vectors: case {id:?} lacks 'bytes'"))?
                        as usize,
                    sha3_256: field("sha3_256")?,
                });
            }
            cases.insert(id.clone(), digests);
        }
        Ok(Self { cases })
    }

    /// Number of cases with expected vectors.
    #[must_use]
    pub fn len(&self) -> usize {
        self.cases.len()
    }

    /// Whether there are no expected vectors.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cases.is_empty()
    }
}

/// Result of one case.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaseReport {
    /// Case identifier.
    pub id: String,
    /// `None` when the case passed, otherwise why it failed.
    pub failure: Option<String>,
    /// Digests of the outputs the backend produced.
    pub outputs: Vec<Digest>,
}

/// Result of a run of the suite.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    /// Name of the backend that ran the cases.
    pub backend: String,
    /// One entry per case, in suite order.
    pub cases: Vec<CaseReport>,
}

impl Report {
    /// Number of cases that passed.
    #[must_use]
    pub fn passed(&self) -> usize {
        self.cases.iter().filter(|c| c.failure.is_none()).count()
    }

    /// Number of cases that failed.
    #[must_use]
    pub fn failed(&self) -> usize {
        self.cases.len() - self.passed()
    }

    /// Whether at least one case ran and none failed.
    #[must_use]
    pub fn all_pass(&self) -> bool {
        !self.cases.is_empty() && self.failed() == 0
    }

    /// The report as canonical JSON followed by a newline. It contains no
    /// timing or platform data.
    #[must_use]
    pub fn to_json(&self) -> String {
        let cases = self
            .cases
            .iter()
            .map(|c| {
                let mut entries = vec![
                    ("id", Value::Str(c.id.clone())),
                    (
                        "outputs",
                        Value::Array(c.outputs.iter().map(Digest::value).collect()),
                    ),
                    (
                        "status",
                        Value::Str(if c.failure.is_none() { "PASS" } else { "FAIL" }.into()),
                    ),
                ];
                if let Some(why) = &c.failure {
                    entries.push(("detail", Value::Str(why.clone())));
                }
                object(entries)
            })
            .collect();
        let doc = object([
            ("backend", Value::Str(self.backend.clone())),
            ("cases", Value::Array(cases)),
            ("numerics_version", Value::Int(i64::from(NUMERICS_VERSION))),
            ("suite", Value::Str(SUITE.into())),
            (
                "summary",
                object([
                    ("cases", Value::Int(self.cases.len() as i64)),
                    ("fail", Value::Int(self.failed() as i64)),
                    ("pass", Value::Int(self.passed() as i64)),
                ]),
            ),
            ("vqs_version", Value::Int(i64::from(VQS_VERSION))),
        ]);
        let mut text = doc
            .canonical()
            .expect("report values are strings and small integers");
        text.push('\n');
        text
    }
}

/// Run one case and digest its outputs.
fn digests(case: &Case, backend: &mut dyn StateBackend) -> Result<Vec<Digest>, String> {
    let outputs = case.run(backend).map_err(|e| e.to_string())?;
    Ok(outputs
        .iter()
        .map(|o| Digest {
            name: o.name.into(),
            dtype: o.dtype.into(),
            bytes: o.bytes.len(),
            sha3_256: hex(&sha3_256(&o.bytes)),
        })
        .collect())
}

fn compare(expected: Option<&Vec<Digest>>, got: &[Digest]) -> Option<String> {
    let Some(expected) = expected else {
        return Some("no expected vector for this case".into());
    };
    if expected.len() != got.len() {
        return Some(format!(
            "{} outputs, expected {}",
            got.len(),
            expected.len()
        ));
    }
    for (e, g) in expected.iter().zip(got) {
        if e.name != g.name || e.dtype != g.dtype {
            return Some(format!(
                "output {}:{} where {}:{} was expected",
                g.name, g.dtype, e.name, e.dtype
            ));
        }
        if e.bytes != g.bytes {
            return Some(format!(
                "output {}: {} bytes, expected {}",
                g.name, g.bytes, e.bytes
            ));
        }
        if e.sha3_256 != g.sha3_256 {
            return Some(format!(
                "output {}: SHA3-256 {} differs from expected {}",
                g.name, g.sha3_256, e.sha3_256
            ));
        }
    }
    None
}

/// Run the cases whose identifier starts with `prefix` (all cases for the
/// empty prefix) on `backend` and compare them with `expected`.
pub fn run(backend: &mut dyn StateBackend, expected: &Expected, prefix: &str) -> Report {
    let name = backend.name().to_owned();
    let cases = suite()
        .iter()
        .filter(|c| c.id.starts_with(prefix))
        .map(|case| match digests(case, backend) {
            Ok(outputs) => CaseReport {
                id: case.id.clone(),
                failure: compare(expected.cases.get(&case.id), &outputs),
                outputs,
            },
            Err(why) => CaseReport {
                id: case.id.clone(),
                failure: Some(format!("backend error: {why}")),
                outputs: Vec::new(),
            },
        })
        .collect();
    Report {
        backend: name,
        cases,
    }
}

/// Run the whole suite against the vectors compiled into the library.
pub fn selftest(backend: &mut dyn StateBackend) -> Result<Report, String> {
    let expected = Expected::embedded()?;
    let report = run(backend, &expected, "");
    if expected.len() != report.cases.len() {
        return Err(format!(
            "{} expected vectors for {} cases: regenerate the vectors",
            expected.len(),
            report.cases.len()
        ));
    }
    Ok(report)
}

/// Produce the `expected.json` document from `backend` (the CPU reference):
/// one case per line, in suite order.
pub fn generate(backend: &mut dyn StateBackend) -> Result<String, String> {
    let mut text = format!(
        "{{\"suite\":\"{SUITE}\",\"vqs_version\":{VQS_VERSION},\"numerics_version\":{NUMERICS_VERSION},\"cases\":{{\n"
    );
    let cases = suite();
    for (i, case) in cases.iter().enumerate() {
        let outputs = digests(case, backend).map_err(|e| format!("case {}: {e}", case.id))?;
        let line = Value::Array(outputs.iter().map(Digest::value).collect())
            .canonical()
            .map_err(|e| e.to_string())?;
        let id = Value::Str(case.id.clone())
            .canonical()
            .map_err(|e| e.to_string())?;
        text.push_str(&format!(
            "{id}:{line}{}\n",
            if i + 1 < cases.len() { "," } else { "" }
        ));
    }
    text.push_str("}}\n");
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vikshep_statevec::CpuBackend;

    fn cpu() -> CpuBackend {
        CpuBackend::new(1).unwrap()
    }

    #[test]
    fn generated_vectors_parse_and_pass() {
        let text = generate(&mut cpu()).unwrap();
        let expected = Expected::parse(&text).unwrap();
        assert_eq!(expected.len(), suite().len());
        let report = run(&mut cpu(), &expected, "");
        assert!(report.all_pass(), "{} failures", report.failed());
        assert!(report.to_json().ends_with("\"vqs_version\":1}\n"));
    }

    #[test]
    fn committed_vectors_match_the_cpu_reference() {
        let report = selftest(&mut cpu()).unwrap();
        let failures: Vec<&CaseReport> = report
            .cases
            .iter()
            .filter(|c| c.failure.is_some())
            .collect();
        assert!(
            failures.is_empty(),
            "{} of {} cases fail; first: {} ({})",
            failures.len(),
            report.cases.len(),
            failures[0].id,
            failures[0].failure.as_deref().unwrap_or_default()
        );
    }

    #[test]
    fn a_wrong_backend_is_caught() {
        // A backend that conjugates the state on read: wrong bytes, same
        // lengths.
        struct Wrong(CpuBackend);
        impl StateBackend for Wrong {
            fn name(&self) -> &str {
                "wrong"
            }
            fn n_qubits(&self) -> u32 {
                self.0.n_qubits()
            }
            fn reset(&mut self, n: u32) -> Result<(), vikshep_qcore::QError> {
                self.0.reset(n)
            }
            fn load(&mut self, n: u32, a: &[f32]) -> Result<(), vikshep_qcore::QError> {
                self.0.load(n, a)
            }
            fn apply(
                &mut self,
                m: &[vikshep_qcore::C32],
                t: &[u32],
                c: &[u32],
            ) -> Result<(), vikshep_qcore::QError> {
                self.0.apply(m, t, c)
            }
            fn read(&self) -> Result<Vec<f32>, vikshep_qcore::QError> {
                let mut s = self.0.read()?;
                for im in s.iter_mut().skip(1).step_by(2) {
                    *im = -*im;
                }
                Ok(s)
            }
        }
        let expected = Expected::parse(&generate(&mut cpu()).unwrap()).unwrap();
        let report = run(&mut Wrong(cpu()), &expected, "gate/");
        assert_eq!(report.backend, "wrong");
        assert!(report.failed() > 0);
        assert!(!report.all_pass());
        assert!(report.to_json().contains("\"status\":\"FAIL\""));
    }

    /// A second implementation of VQS-1 section 5, written from the
    /// definition (out of place, one amplitude at a time), whose arithmetic
    /// can be swapped for a deliberately non-conforming one.
    struct Variant {
        n: u32,
        state: Vec<f32>,
        mul: fn(f32, f32) -> f32,
        add: fn(f32, f32) -> f32,
        sub: fn(f32, f32) -> f32,
        skip_zero_entries: bool,
    }

    impl Variant {
        fn conforming() -> Self {
            Self {
                n: 1,
                state: vec![1.0, 0.0, 0.0, 0.0],
                mul: vikshep_qcore::arith::mul,
                add: vikshep_qcore::arith::add,
                sub: vikshep_qcore::arith::sub,
                skip_zero_entries: false,
            }
        }
    }

    impl StateBackend for Variant {
        fn name(&self) -> &str {
            "variant"
        }
        fn n_qubits(&self) -> u32 {
            self.n
        }
        fn reset(&mut self, n: u32) -> Result<(), vikshep_qcore::QError> {
            self.state = vikshep_qcore::reference::zero_state(n)?;
            self.n = n;
            Ok(())
        }
        fn load(&mut self, n: u32, a: &[f32]) -> Result<(), vikshep_qcore::QError> {
            vikshep_qcore::reference::check_state(n, a)?;
            self.state = a.iter().map(|&v| vikshep_qcore::arith::flush(v)).collect();
            self.n = n;
            Ok(())
        }
        fn apply(
            &mut self,
            matrix: &[vikshep_qcore::C32],
            targets: &[u32],
            controls: &[u32],
        ) -> Result<(), vikshep_qcore::QError> {
            use vikshep_qcore::C32;
            let dim = 1usize << targets.len();
            let target_mask = targets.iter().fold(0usize, |m, &t| m | (1 << t));
            let control_mask = controls.iter().fold(0usize, |m, &c| m | (1 << c));
            let old = self.state.clone();
            for i in 0..old.len() / 2 {
                if i & control_mask != control_mask {
                    continue;
                }
                let row = targets
                    .iter()
                    .enumerate()
                    .fold(0, |r, (b, &t)| r | (((i >> t) & 1) << b));
                let mut acc: Option<C32> = None;
                for col in 0..dim {
                    let j = targets
                        .iter()
                        .enumerate()
                        .fold(i & !target_mask, |v, (b, &t)| v | (((col >> b) & 1) << t));
                    let m = matrix[row * dim + col];
                    if self.skip_zero_entries && m.re == 0.0 && m.im == 0.0 {
                        continue;
                    }
                    let x = C32::new(old[2 * j], old[2 * j + 1]);
                    let term = C32::new(
                        (self.sub)((self.mul)(m.re, x.re), (self.mul)(m.im, x.im)),
                        (self.add)((self.mul)(m.re, x.im), (self.mul)(m.im, x.re)),
                    );
                    acc = Some(match acc {
                        None => term,
                        Some(a) => C32::new((self.add)(a.re, term.re), (self.add)(a.im, term.im)),
                    });
                }
                let y = acc.unwrap_or(C32::ZERO);
                self.state[2 * i] = y.re;
                self.state[2 * i + 1] = y.im;
            }
            Ok(())
        }
        fn read(&self) -> Result<Vec<f32>, vikshep_qcore::QError> {
            Ok(self.state.clone())
        }
    }

    #[test]
    fn an_independent_kernel_with_the_specified_arithmetic_conforms() {
        let report = selftest(&mut Variant::conforming()).unwrap();
        assert!(report.all_pass(), "{} failures", report.failed());
    }

    #[test]
    fn the_suite_rejects_common_non_conforming_arithmetic() {
        let expected = Expected::embedded().unwrap();
        let failures = |mut v: Variant| run(&mut v, &expected, "").failed();

        // No flush at all: IEEE gradual underflow, as an unflagged CPU gives.
        let no_flush = failures(Variant {
            mul: |a, b| a * b,
            add: |a, b| a + b,
            sub: |a, b| a - b,
            ..Variant::conforming()
        });
        // Flush with tininess judged after rounding (VDS-1.1, not VDS-1.2).
        let after_rounding = failures(Variant {
            mul: |a, b| vikshep_qcore::arith::flush(a * b),
            ..Variant::conforming()
        });
        // Skipping zero matrix entries: changes the sign of zero results.
        let skipping = failures(Variant {
            skip_zero_entries: true,
            ..Variant::conforming()
        });
        println!(
            "failing cases: no flush {no_flush}, after-rounding tininess {after_rounding}, \
             skipped zero entries {skipping}"
        );
        assert!(no_flush > 0);
        assert!(after_rounding > 0);
        assert!(skipping > 0);
    }

    #[test]
    fn prefix_selects_a_family() {
        let expected = Expected::parse(&generate(&mut cpu()).unwrap()).unwrap();
        let report = run(&mut cpu(), &expected, "ctrl/");
        assert!(report.all_pass());
        assert!(report.cases.iter().all(|c| c.id.starts_with("ctrl/")));
        assert!(!run(&mut cpu(), &expected, "no-such-family/").all_pass());
    }
}
