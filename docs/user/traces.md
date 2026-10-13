# Independent trace validation

Validate a saved checker witness against the same specification and configuration:

```sh
dive check docs/user/examples/Counter.tla --config docs/user/examples/CounterViolation.cfg --compat=tlc --seed 7 --fp 0 --output json > counter-result.json
# The expected invariant violation exits 12; retain the document.
python3 -c 'import json; r=json.load(open("counter-result.json")); json.dump(r["counterexample"]["trace"], open("counter-trace.json", "w"))'
dive validate-trace docs/user/examples/Counter.tla --config docs/user/examples/CounterViolation.cfg --trace counter-trace.json --compat=tlc --seed 7 --fp 0 --output json
```

The validator evaluates Init and the original next-state relation as predicates
over supplied concrete states. It is independent of the checker's successor
enumerator. Every state must assign every spec variable; missing/partial states
are not completed by guessing. Trace format version 1 normalizes value JSON and has known representation
gaps. Successful decoding is separate from semantic validation. TLC dumpTrace
JSON is also an accepted input format within the validator's documented boundary.

Rust applications can validate their own observed state sequence:

```rust
use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use serde_json::json;

let source = "---- MODULE Steps ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nNext == x' = x + 1\n====\n";
let model = SpecModel::from_source("Steps.tla", source,
    "INIT Init\nNEXT Next\n", &TraceOptions::default())?;
let trace = TraceDoc::behavior(vec![json!({"x": 0}), json!({"x": 1})])?;
let result = model.validate(&trace);
assert_eq!(result.validation, Validation::Valid);
```

Use a `Result`-returning surrounding function and add `serde_json` to the caller's
dependencies. `Behavior` checks the supplied finite execution; it does not claim
an invariant counterexample or exhaustive model proof. Counterexample endings
and periodic lassos carry separate claims and closing-edge obligations.

The lower-level `dive_trace` API exposes `validate(&model, &trace)` and
`validate_doc(&model, &document, &model_values)`. Neither takes a random replay
token; a random draw during validation remains explicitly unsupported.

`SpecModel::controls` checks a known shortest counterexample, its document
round trip, and mutations of its states, claim and action labels. Every
generated mutant must be invalid; a valid or unsupported mutant is a control
failure. The harness runs these checks with `dive-conformance trace-controls`.
Random redraw mutations are unavailable while random validation is unsupported.

Validation distinguishes valid, invalid and explicitly refused. Random draws,
search-dependent register access, unavailable finite deadlock/ENABLED covers,
effects and unknown closing evidence do not become `valid`. Supply the original
seed/fp for run-dependent model binding and use the checking profile. The in-memory lasso back edge is zero-based; checker
summary `back_to` display is one-based. Prefer the supplied trace document over reconstructing a lasso from displayed
states, and inspect its decoder and independent-validation disposition.

See the [API rustdoc](../../crates/dive-api/src/trace.rs),
[temporal contract](../temporal-api.md) and [validator independence ADR](../decisions/0016-trace-validator-independence.md).

A known mixed-set limitation remains: a checker can emit a JSON set containing
a sequence before a record that the decoder rejects at the shape check, in
either profile. Such a result receives no validation credit.
[Follow-up #190](https://github.com/Chelis-Lang/dive/issues/190) tracks this boundary.

Under `--compat=tlc`, strings and model values compare in the order TLC first
encountered them. The validator knows that order for every name interned when
the model is bound. If reading the trace or checking it creates any other
name (a string the specification never mentions, or one an operator such as
`ToString` or `Tail` builds on these states), the result is
`unsupported: runtime string encounter order`, never `valid` or `invalid`.
A check that draws a random value (`RandomElement`, `Randomization`) is
likewise refused by name.

Before entering an `INSTANCE ... WITH` substitution, whose strict bindings are
evaluated eagerly, the validator refuses a binding from another model's
context or one whose effects annotation is not pure. It trusts the IR's
effects annotations; it does not re-prove them.


### Initial-predicate level

Init runs at startup level 0; generated-state invariants and implied initial properties run at level 1. The independent validator uses a distinct initial context and preserves ordinary state/action contexts thereafter. Random and register refusals remain unchanged.
