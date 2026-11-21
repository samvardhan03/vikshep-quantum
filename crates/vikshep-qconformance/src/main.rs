//! Command-line runner of the VQS-1 conformance suite.
//!
//! ```text
//! vikshep-qconformance run [--threads N] [--prefix P] [--report FILE]
//! vikshep-qconformance generate [--out FILE]
//! vikshep-qconformance list
//! ```
//!
//! `run` checks the CPU reference against the committed vectors and prints
//! the report (or writes it to FILE); it exits with status 1 if any case
//! fails. `generate` rewrites the expected vectors from the CPU reference.

use std::process::ExitCode;

use vikshep_qconformance::cases::suite;
use vikshep_qconformance::runner::{Expected, generate, run};
use vikshep_statevec::CpuBackend;

const DEFAULT_VECTORS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../conformance/vectors/v1/expected.json"
);

const USAGE: &str = "usage:
  vikshep-qconformance run [--threads N] [--prefix P] [--report FILE]
  vikshep-qconformance generate [--out FILE]
  vikshep-qconformance list";

fn option(args: &[String], name: &str) -> Result<Option<String>, String> {
    match args.iter().position(|a| a == name) {
        None => Ok(None),
        Some(i) => args
            .get(i + 1)
            .cloned()
            .map(Some)
            .ok_or_else(|| format!("{name} needs a value")),
    }
}

fn backend(args: &[String]) -> Result<CpuBackend, String> {
    let b = CpuBackend::new(1).map_err(|e| e.to_string())?;
    match option(args, "--threads")? {
        None => Ok(b),
        Some(t) => {
            let threads: usize = t
                .parse()
                .map_err(|_| format!("--threads: {t:?} is not a number"))?;
            Ok(b.with_threads(threads))
        }
    }
}

fn main_inner(args: &[String]) -> Result<bool, String> {
    match args.first().map(String::as_str) {
        Some("run") => {
            let expected = Expected::embedded()?;
            let prefix = option(args, "--prefix")?.unwrap_or_default();
            let report = run(&mut backend(args)?, &expected, &prefix);
            let json = report.to_json();
            match option(args, "--report")? {
                Some(path) => std::fs::write(&path, &json).map_err(|e| format!("{path}: {e}"))?,
                None => print!("{json}"),
            }
            for case in report.cases.iter().filter(|c| c.failure.is_some()) {
                eprintln!(
                    "FAIL {}: {}",
                    case.id,
                    case.failure.as_deref().unwrap_or_default()
                );
            }
            eprintln!(
                "{}: {} cases, {} pass, {} fail",
                report.backend,
                report.cases.len(),
                report.passed(),
                report.failed()
            );
            Ok(report.all_pass())
        }
        Some("generate") => {
            let path = option(args, "--out")?.unwrap_or_else(|| DEFAULT_VECTORS.into());
            let text = generate(&mut backend(args)?)?;
            std::fs::write(&path, &text).map_err(|e| format!("{path}: {e}"))?;
            eprintln!("wrote {} cases to {path}", suite().len());
            Ok(true)
        }
        Some("list") => {
            for case in suite() {
                println!("{}", case.id);
            }
            Ok(true)
        }
        _ => Err(USAGE.into()),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match main_inner(&args) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(2)
        }
    }
}
