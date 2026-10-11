# Replaying tuple-valued state variables

The independent validator can prove a bounded tuple invariant from the model
before replaying a counterexample. This supports sequence operations on a state
variable without using the proposed trace as type evidence. The proof does not
construct successors and does not change the checker’s sequential fallback.

For example, save this as `TupleHistory.tla`:

```tla
---- MODULE TupleHistory ----
EXTENDS Naturals, Sequences, TLC
VARIABLES x, s
Init == x=0 /\ s = <<0,1>>
Next == x'=1 /\ s'=Tail(s)
Inv == x=0
====
```

Use this `TupleHistory.cfg`:

```text
INIT Init
NEXT Next
INVARIANT Inv
```

```sh
dive check TupleHistory.tla --compat=tlc --seed 0 --fp 0 --trace-format representation-v2 --output json > result.json
# The invariant violation exits 12; preserve its JSON report.
dive validate-trace TupleHistory.tla --compat=tlc --seed 0 --fp 0 --trace result.json --output json
```

The clean pinned oracle reports the two-state `Inv` violation. The variable `s`
starts as `<<0,1>>` and becomes `<<1>>`. Replay checks Init, the actual Next
relation and the invariant ending. Changing the final `x` to 99 must be Invalid.
Finding a tuple in the report itself never establishes the invariant.

The executable Rust guide uses the same checked module and public API:

```sh
cargo run --locked -p dive-api --example inductive_tuple_replay
```

## Supported proof rules

A necessary equality in Init must establish a tuple independently of state
variables. Every disjunct and both branches of an initial conditional must
establish it. Next must preserve the tuple on every branch through an equality,
`UNCHANGED`, or a supported tuple operation. Conjunctions use necessary facts;
disjunctions and conditionals require all branches. Definitions, actual lazy
parameter scopes and LET/substitution contexts retain their bound meaning.

Configured operator constants redirected to known native sequence operators use their
authoritative binding metadata. No provider is forced to discover an operator.

The current operation slice includes tuple literals, concatenation, `Tail` and
`SubSeq`. Mutually dependent tuple variables are retained only if the entire
candidate set is inductive; removing one unproved variable removes dependent
proofs as well. The separate all-role audit still rejects nested runtime-name
construction, including `<<ToString(x)>>` even on an off-path branch.

Proof construction is limited to 256 variables, 32,768 predicate visits and
depth 64, with 256 visits per operand-shape proof. These are bounded inspection
policies, not a total search memory guarantee. Exhaustion means refusal.

These facts describe accepted histories. A malformed supplied initial value can
reach a sequence operation before a later necessary equality rejects it; the
validator still checks that equality. This is not a promise of zero runtime
effects while rejecting every malformed word. The proof construction itself
never evaluates expressions or interns names.

## Remaining refusals

A string initial value, a tuple/string branch, an omitted next-state assignment,
opaque bindings, unproved recursive calls or unsupported construction rules
retain the runtime string-order refusal. In particular, one successful witness
prefix cannot justify an assignment on an unvisited branch.

The public independent enabledness and lasso predicate APIs may receive arbitrary
states without a validated predecessor chain. They do **not** inherit this
whole-word history proof. The fixed-projection continuation follows `UNCHANGED vars` through nested
fixed tuple aliases and lazy parameters, LET and substitution contexts.
A variable must occupy an established fixed projection; finding it somewhere
in every conditional branch is insufficient. Conditional tuple permutations,
opaque slots and recursive aliases remain refused. See [ADR 0071](decisions/0071-fixed-tuple-projection-replay.md)
for the rule. Focused controls pass; fresh runtime qualification is pending.
General runtime-name provenance and all procedure stack histories remain unproved; issue #124 stays open. Canonical-profile
behavior and TLC-compatible evaluation remain available under their existing
settings. This is an admission improvement, not a new result-changing default.

The implementation is under integration validation. Focused tests cover actual
and altered words, both initial branches, off-path string construction, missing
assignments, wrappers and the arbitrary-state API boundary. Whole-workspace and
fresh receiving CLI results are required before publication.

See [the literal tuple admission](tuple-only-trace-replay.md) and
[ADR 0067](decisions/0067-inductive-tuple-replay.md).
