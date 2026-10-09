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
zero-arity definitions, pure constant/state-argument operators, finite constant-domain quantifiers with explicit sets
or intervals (including tuple bindings), direct `ENABLED`, and weak/strong fairness over supported
pure actions. Action/temporal arguments, higher-order calls, nonconstant/unbounded quantification, dynamic scopes, effects and
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
retains its temporal refusal unless final/short checking is explicitly selected.


Original configured PROPERTY formulas are retained for attribution. A universal
property may split into several checking roles, but its counterexample claim is
still the original configuration name; all roles are checked without guessing
which repeated internal name means which binding.

Pure `IF` formulas use their guard's truth at each position, including temporal
guards and nested conditionals. Action-level guards and temporal `CASE` remain
explicit refusals: the pinned TLC also rejects them during liveness planning.
A formula with an effectful branch is refused before planning executes it.

## Command-line access

The same backend is available explicitly:

```sh
dive check Model.tla --lncheck final --lasso short --store exact --seed 17 --output json
```

Both flags are required. The model must have a temporal property; ordinary
safety checks use the existing command without these flags. Supported final
checking returns success or independently validated periodic counterexamples;
unsupported formulas, missing independent covers and limits return no verdict.
Safety violations found during exploration retain their full original report.

Opt-in requests use JSON schema 2. Ordinary checks retain schema 1. A temporal
violation includes `properties`, a lossless `counterexample.trace`, a 1-based
`counterexample.back_to`, and `counterexample.validation: "valid"`. The trace
document itself retains its own schema and 0-based back edge. Save the trace
field and replay it with `dive validate-trace` against the same spec/config.

`--compat=tlc` with these explicit overrides is labeled `tlc+overrides`. It
selects TLC's values and reports temporal violations with exit 13, but does
not claim TLC-exact lasso selection, finite-prefix stopping, temporal error
classes or periodic scheduling. The `tlc` class field stays null for temporal
holds/violations. Default-profile violations exit 10. The ordinary strict
conformance path retains its temporal refusal until those counterparts land.

Capture currently runs inline even when more workers were requested; actual
width and its reason are reported. Bounds are counts of retained entries,
not a hard byte-memory limit; they exclude preceding model binding.

Constant arguments are inspected for purity before evaluation and become value
bindings in each call's retained scope. For example, `Visit(i) == []<>(x=i)`
can be called as `Visit(2)` or from a constant-domain temporal quantifier.
Repeated calls with different values keep separate contexts. The planner and
independent word validator each check their own scoped arguments and work/depth
bounds. Hidden checker-register reads are refused;
parameterized actions whose ENABLED proof lacks an independent finite cover
remain validation refusals.

Independent ENABLED/fairness proofs now follow pure constant-argument action
calls and value-bound locals. Covers retain each call's scalar/tuple context,
and every witness is tested against the original relation. Missing primed
variable covers, lazy state arguments in the independent finite-cover search,
and higher-order calls still refuse.
The `SfDisabled` probe specifically requires false ENABLED on its stuttering
cycle; a recurring action cannot stand in for this proof.

Pure state arguments now work too: `Live(p) == []<>p` can take `Live(x)`,
including nested forwarding, tuple arguments and state guards. State bindings
remain lazy and retain their caller context; priming inside the callee reads
the successor value. Planning checks instantiated levels before classifying
an atom or folding a constant. The independent word evaluator resolves those
contexts separately. Action/temporal arguments and higher-order operators
remain named refusals.

Finite constant-domain quantifiers inside state/action predicates now retain
all instantiated body scopes during planning and independent word validation.
This includes quantified actions in WF/SF. Independent necessary covers combine
existential alternatives and universal constraints, then check the entire original
relation for each assignment. Nested domains may read already bound outer values;
state-dependent domains remain refused by temporal preflight, while independent
ENABLED can decide a finite domain using its concrete current state. Primed and
infinite domains, effects, missing covers and explicit budgets remain refusals.

Pure `LET` expressions retain their enclosing scope in both planning and
independent validation. Zero-arity values stay lazy, including captured state
and priming; local operators keep their parent frame. Independent covers follow
used LET values and prove action-call arguments are state-independent before
binding them as constants. Used effects/checker reads, missing covers and work
budgets still refuse. Six pinned `Lt*` models match final truth/counts in both
profiles/stores, with every returned periodic claim independently validated.

Pure INSTANCE value/variable bindings retain captured caller scopes in temporal
predicates, including nested substitutions, quantifiers and priming. Operator
substitutions and independently proving disabledness through an INSTANCE remain
named refusals. Effects and checker reads are rejected before evaluation.

Pure finite constant-domain set maps, filters and function constructors retain
local bindings and captured state in temporal predicates. State-dependent
domains and recursive constructors remain explicit refusals. See the `Bc*`
probes in `crates/dive-api/tests/fixtures/temporal-constructors/`.
