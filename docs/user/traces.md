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
are not completed by guessing. Version 1 normalizes value JSON and has known
representation gaps. Opt-in version 2 retains admitted stored representations
with checked construction witnesses; successful transport is still not semantic
validation. TLC dumpTrace JSON is also an accepted input format within the
validator's documented boundary.

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
summary `back_to` display is one-based. Prefer the supplied trace document over reconstructing a lasso from displayed
states, and inspect its decoder and independent-validation disposition.

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

Opt-in `--trace-format representation-v2` addresses the admitted forward mixed
record/tuple set and nonliteral function transport boundary. Version1 stays the
default with its existing observed failures; it is not silently retagged as
lossless. See the [schema, runnable CLI/Rust guide and limits](../trace-representation.md).
Reverse construction, arbitrary deferred/symbolic values and Random/VIEW shape
extensions remain unsupported. No prior shape failure earns new replay credit.

The current v2 selection separately records 72 independently valid originals
and 216 semantic altered-word rejections, including both profiles, both stores
and 1/4/8 workers. An encoder may still refuse the stored shape: inspect
`counterexample.trace_encoding`, whose `semantic_validation` remains false even
when encoding succeeds. Unknown/mismatched receiving-model name tokens and
constructor-changing nested Raw representations refuse before Model-based
emission. Read the [current scope and evidence](../trace-representation.md#current-scoped-evidence)
without treating transport admission as a certificate.

For TLC-profile sequence calls, [literal tuple admission](../tuple-only-trace-replay.md)
and [bounded tuple-variable histories](../inductive-tuple-trace-replay.md)
prove operand shape without using the report itself as evidence. The latter
requires Init and every Next branch to establish/preserve the invariant.
A [bounded finite function-range proof](../indexed-stack-trace-replay.md) additionally admits tuple stacks indexed by process owners when Init and every Next branch establish/preserve the range. Arbitrary-state APIs and general saved procedure stacks remain refused.

Public serialized IR with Strict substitution expressions requires independent
pre-entry proof from the actual operand and captured scope. Closed literals
and safe forwarding are admitted; calls and unproved operands remain refused. See
[strict context admission](strict-context-admission.md) for the Rust endpoint
contract and unchanged lazy/ordinary-evaluator behavior.
