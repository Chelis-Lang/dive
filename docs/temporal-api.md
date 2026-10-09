# Final temporal checking from Rust

`dive_api::temporal::check_final` checks the supported pure temporal subset
once the safety graph is completely explored. It returns either a holding
result, an independently validated periodic counterexample, or the safety
engine's earlier stop. Planning and validation refusals return an error.

```rust
use std::sync::Arc;
use dive_api::temporal::{check_final, Outcome};
use dive_check::CheckOptions;
use dive_liveness::analyze::Limits;

let source = "---- MODULE M ----\nVARIABLE x\nInit == x = TRUE\nNext == x' = ~x\nSpec == Init /\\ [][Next]_x /\\ WF_x(Next)\nP == <>[]x\n====\n";
let analysis = dive_sema::analyze_source("M.tla", source, &dive_sema::Options::default());
let ir = Arc::new(dive_ir::lower(&analysis)?);
let config = dive_config::parse("SPECIFICATION Spec\nPROPERTY P\n")
    .into_config().ok_or("invalid config")?;
let report = check_final(&ir, &config, &CheckOptions::dive(17), Limits::default())?;

match report.outcome() {
    Outcome::Violation(v) => {
        println!("{}", v.document().to_json());
        assert_eq!(v.validation().validation, dive_trace::Validation::Valid);
    }
    Outcome::Holds(evidence) => println!("{} distinct states", evidence.stats.distinct),
    Outcome::Safety(result) => println!("earlier safety result: {:?}", result.verdict),
}
```

The function binds a fresh model for each run and a separate model for trace
validation. Returned counterexamples name **all** configured temporal
properties they falsify. Their trace documents preserve exact values and
include the closing edge. Both the in-memory word and its serialized form must
pass validation before the API returns a violation.

The supported language includes pure boolean combinations, `[]`, `<>`, `~>`,
zero-arity definitions, finite constant-domain quantifiers with explicit sets
or intervals (including tuple bindings), direct `ENABLED`, and weak/strong fairness over supported
pure actions. Temporal parameters, nonconstant/unbounded quantification, dynamic scopes, effects and
checker-register reads remain explicit refusals. The independent validator
needs a finite successor cover to prove some ENABLED/fairness cases; missing
covers return a validation refusal even if analysis finds a candidate.

Capture runs inline and reports the actual width and the requested width.
The limits bound formula entries, graph states/edges/generated work, labels and
product construction; these are **entry/work counts**, not byte limits or a
wall-clock deadline, and do not bound the preceding binding stage. Fingerprint mode retains the safety engine's collision
qualifications; exact mode stores full states.

This API uses final checking and deterministic short-lasso selection. Passing
`CheckOptions::tlc` selects TLC's value/arithmetic/ordering semantics, but does
not select TLC's exact lasso, periodic schedule or evaluation-error timing.
It does not replace the strict TLC conformance path. Ordinary `dive check`
continues to refuse temporal models while that integration is completed.


Original configured PROPERTY formulas are retained for attribution. A universal
property may split into several checking roles, but its counterexample claim is
still the original configuration name; all roles are checked without guessing
which repeated internal name means which binding.
