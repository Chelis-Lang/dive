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
are not completed by guessing. The tagged value format distinguishes sets,
sequences, records, functions, strings and model values. It does not preserve
every TLC construction flag or function representation; decoding applies
construction/comparability rules. Serialization alone never proves that the
word can be replayed. TLC dumpTrace JSON is also an accepted input
format within the validator's documented boundary.

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

Validation distinguishes valid, invalid and explicitly refused. Named random
replay restrictions, unavailable finite deadlock/ENABLED covers, effects and
unknown closing evidence do not become `valid`. Supply the original seed/fp and
profile when required. The in-memory lasso back edge is zero-based; checker
summary `back_to` display is one-based. Prefer the tagged trace document to
reconstructing a lasso from the displayed state list.

See the [API rustdoc](../../crates/dive-api/src/trace.rs),
[temporal contract](../temporal-api.md) and [validator independence ADR](../decisions/0016-trace-validator-independence.md).

A known mixed-set limitation remains: a checker can emit canonical JSON set
order containing a sequence before a record, which the current decoder
rejects with `result: invalid` at the shape check. This happens in both profiles;
it is decoder admission failure, not semantic replay of the word. The current
VIEW normalization selection retains 36 such original shape failures and 108
mutant shape failures, separately from 96 independently valid words and 288
semantic mutant rejections. Twelve originals are separately supplied
default-reverse candidates, not emitted checker certificates. No shape failure
receives validation credit. The separate homogeneous checkpoint matrix proves
216 valid original/recovered words and 216 altered-word rejections.
[Follow-up #190](https://github.com/Chelis-Lang/dive/issues/190) and the
[source-bound campaign](../../conformance/probes/view-concrete-normalization/README.md)
retain the exact outputs and incomplete coverage.
