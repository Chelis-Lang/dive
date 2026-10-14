//! The permitted internal dependency graph between `dive-*` crates.
//!
//! This encodes the charter's dependency structure: syntax feeds semantics,
//! semantics feeds the IR, every backend consumes the IR, and `dive-value` /
//! `dive-eval` underlie checking, liveness, simulation and trace validation.
//! In particular `dive-trace` may not depend on `dive-check`, `dive-liveness`,
//! `dive-sim` or `dive-store`, so the trace validator stays a genuine second
//! check. `dive-model`, the binding stage, sits between the evaluator and
//! every backend (ADR 0013 §8). `dive-trace` must also build without
//! `dive-eval`'s `enumerate` feature, the successor enumerator (ADR 0016 §2);
//! see [`ENUMERATE_FREE`]. Changing this table is an architectural decision:
//! record an ADR.

/// `(crate, internal crates it may depend on directly)`.
pub const ALLOWED: &[(&str, &[&str])] = &[
    ("dive-syntax", &[]),
    ("dive-sema", &["dive-syntax"]),
    ("dive-ir", &["dive-syntax", "dive-sema"]),
    ("dive-value", &[]),
    ("dive-config", &["dive-syntax", "dive-value"]),
    ("dive-eval", &["dive-ir", "dive-value"]),
    ("dive-stdlib", &["dive-ir", "dive-value", "dive-eval"]),
    ("dive-store", &["dive-value"]),
    // The binding stage, `SpecProcessor` (ADR 0013 §8; open question 11).
    (
        "dive-model",
        &[
            "dive-ir",
            "dive-value",
            "dive-config",
            "dive-eval",
            "dive-stdlib",
        ],
    ),
    (
        "dive-check",
        &[
            "dive-ir",
            "dive-value",
            "dive-eval",
            "dive-stdlib",
            "dive-config",
            "dive-model",
            "dive-store",
        ],
    ),
    (
        "dive-liveness",
        &[
            "dive-ir",
            "dive-value",
            "dive-eval",
            "dive-stdlib",
            "dive-config",
            "dive-model",
            "dive-store",
            "dive-check",
        ],
    ),
    (
        "dive-sim",
        &[
            "dive-ir",
            "dive-value",
            "dive-eval",
            "dive-stdlib",
            "dive-config",
            "dive-model",
        ],
    ),
    (
        "dive-trace",
        &[
            "dive-ir",
            "dive-value",
            "dive-eval",
            "dive-stdlib",
            "dive-config",
            "dive-model",
        ],
    ),
    ("dive-pluscal", &["dive-syntax", "dive-sema"]),
    (
        "dive-api",
        &[
            "dive-syntax",
            "dive-sema",
            "dive-ir",
            "dive-value",
            "dive-eval",
            "dive-stdlib",
            "dive-config",
            "dive-model",
            "dive-store",
            "dive-check",
            "dive-liveness",
            "dive-sim",
            "dive-trace",
            "dive-pluscal",
        ],
    ),
    ("dive-cli", &["dive-api", "dive-conformance"]),
    (
        "dive-conformance",
        &["dive-api", "dive-value", "dive-config", "dive-trace"],
    ),
    ("dive-bench", &["dive-api", "dive-conformance"]),
];

/// Crates whose standalone build must not enable `dive-eval/enumerate`,
/// directly or through any internal dependency. `cargo xtask check-layering`
/// resolves each one's features ([`crate::features`]), and the workspace gate
/// also builds `dive-trace` on its own, so feature unification across the
/// workspace cannot hide a violation.
pub const ENUMERATE_FREE: &[&str] = &["dive-trace"];

#[cfg(test)]
mod tests {
    use super::ALLOWED;
    use std::collections::{BTreeMap, BTreeSet};

    fn graph() -> BTreeMap<&'static str, &'static [&'static str]> {
        ALLOWED.iter().copied().collect()
    }

    #[test]
    fn every_edge_targets_a_listed_crate() {
        let g = graph();
        for (name, deps) in ALLOWED {
            for dep in *deps {
                assert!(
                    g.contains_key(dep),
                    "{name} -> {dep} names an unlisted crate"
                );
            }
        }
    }

    #[test]
    fn permitted_graph_is_acyclic() {
        fn visit(
            n: &'static str,
            g: &BTreeMap<&'static str, &'static [&'static str]>,
            stack: &mut Vec<&'static str>,
            done: &mut BTreeSet<&'static str>,
        ) {
            if done.contains(n) {
                return;
            }
            assert!(!stack.contains(&n), "cycle: {stack:?} -> {n}");
            stack.push(n);
            for d in g.get(n).copied().unwrap_or_default() {
                visit(d, g, stack, done);
            }
            stack.pop();
            done.insert(n);
        }
        let g = graph();
        let mut done = BTreeSet::new();
        for n in g.keys() {
            visit(n, &g, &mut Vec::new(), &mut done);
        }
    }

    #[test]
    fn trace_validator_is_independent_of_checker() {
        let g = graph();
        let trace = g.get("dive-trace").copied().unwrap_or_default();
        for forbidden in ["dive-check", "dive-liveness", "dive-sim", "dive-store"] {
            assert!(
                !trace.contains(&forbidden),
                "dive-trace may not depend on {forbidden}"
            );
        }
    }
}
