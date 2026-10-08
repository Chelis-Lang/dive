//! dive-sema against SANY's recorded verdicts:
//! - `tests/sany-cases/`: minimal specs of SANY behaviours (see its README);
//! - `tests/error-corpus/`: SANY's own semantic error corpus
//!   (tlaplus `test/tla2sany/semantic/error_corpus` at `94d0c50`, MIT). For
//!   every `E<code>_..._Test.tla` dive must reject and cross-reference
//!   SANY's code `<code>` in one of its diagnostics.
//!
//! The verdict files were produced by the pinned SANY with `-error-codes`
//! (`acceptance/run_sany.py`, `acceptance/verdicts.py`).

use std::path::{Path, PathBuf};

fn dir(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join(name)
}

fn verdicts(name: &str) -> Vec<(String, bool)> {
    let path = dir(name).join("verdicts.txt");
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) => unreachable!("cannot read {}: {e}", path.display()),
    };
    text.lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .filter_map(|l| {
            let (f, v) = l.rsplit_once(' ')?;
            Some((f.to_owned(), v == "accept"))
        })
        .collect()
}

fn check(name: &str) -> Vec<String> {
    let mut failures = Vec::new();
    for (file, accept) in verdicts(name) {
        let path = dir(name).join(&file);
        let a = match dive_sema::analyze_file(&path, &dive_sema::Options::default()) {
            Ok(a) => a,
            Err(e) => {
                failures.push(format!("{file}: cannot read: {e}"));
                continue;
            }
        };
        if a.accepted() != accept {
            let first = a
                .diagnostics()
                .iter()
                .find(|d| d.is_error())
                .map(|d| format!("{} {}", d.code, d.message))
                .unwrap_or_default();
            failures.push(format!(
                "{file}: SANY {}, dive {} {first}",
                if accept { "accepts" } else { "rejects" },
                if a.accepted() { "accepts" } else { "rejects" }
            ));
            continue;
        }
        if let Some(code) = file
            .strip_prefix('E')
            .filter(|_| file.ends_with("_Test.tla"))
            .and_then(|r| r.get(..4))
            .and_then(|c| c.parse::<u16>().ok())
            && !a.diagnostics().iter().any(|d| d.sany == Some(code))
        {
            let got: Vec<String> = a
                .diagnostics()
                .iter()
                .map(|d| format!("{}({:?})", d.code, d.sany))
                .collect();
            failures.push(format!(
                "{file}: no diagnostic cross-references SANY {code}: {got:?}"
            ));
        }
    }
    failures
}

#[test]
fn minimal_cases_match_sany() {
    let f = check("sany-cases");
    assert!(f.is_empty(), "{}", f.join("\n"));
}

#[test]
fn error_corpus_matches_sany_and_its_codes() {
    let f = check("error-corpus");
    assert!(f.is_empty(), "{}", f.join("\n"));
}
