# Finite symbolic safety checking

Use `dive check --strategy symbolic` in a Linux release bundle when a finite TLA+
model has many combinations of the same few fields. It can search the complete
reachable set, check a new invariant against that set, or prove safety by
induction. It reads the original TLA+ module and configuration through Dive's
binder.

## Run a check

Keep the extracted Linux release directory together. The bundled worker and
validator run offline. From the directory containing an existing finite spec:

```sh
dive check Model.tla --config Model.cfg --strategy symbolic --output json
dive check Model.tla --config Model.cfg --strategy symbolic --write-predicate closed.json
dive check Model.tla --config Edited.cfg --strategy symbolic --verify-predicate closed.json
```

The configuration defaults to the module basename with a `.cfg` extension.
Select the finite layout with `--type-invariant NAME`. The explicit symbolic
strategy emits symbolic JSON, including on an unsupported feature. Its exit
codes are 0 for success, 1 for a validated violation, 2 for an unsupported
model or inconclusive proof, 3 for a worker error, and 4 for a resource stop.
`--time-limit` accepts the normal duration units, including milliseconds.
`--max-memory` takes a per-process address-space budget of at least 1 MiB.
The Linux archives contain the worker. A binary-only Cargo installation
requires the companion from a matching release to use this strategy.

For source development on Linux x86-64, build the binder and validator
from the same source as the script. Install the optional CUDD Python binding
in a Python 3.11 virtual environment.

```sh
export CARGO_TARGET_DIR=target
cargo build --release --locked -p dive-api --example symbolic_input
cargo build --release --locked -p dive-cli --bin dive
python3.11 -m venv target/symbolic-python
target/symbolic-python/bin/pip install --only-binary=dd -r scripts/symbolic/requirements.txt
```

The following model delivers each receipt at most once. A stuttering
action keeps the fully delivered state enabled.

```sh
cat > Receipts.tla <<'TLA'
---- MODULE Receipts ----
CONSTANT IDs
VARIABLE got
TypeInvariant == got \subseteq IDs
Init == got = {}
Next == \/ \E i \in IDs \ got : got' = got \cup {i}
        \/ UNCHANGED got
Empty == got = {}
====
TLA
cat > Receipts.cfg <<'CFG'
CONSTANT IDs = {a, b, c}
INIT Init
NEXT Next
INVARIANT TypeInvariant
CFG
PY=target/symbolic-python/bin/python
"$PY" scripts/symbolic/check.py Receipts.tla Receipts.cfg
```

The completed search reports eight distinct states, 21 generated states
including the initial state, and depth four. For comparison, run
`target/release/dive check Receipts.tla --config Receipts.cfg --workers 1 --store exact --compat=tlc --output json`.

## Check edits and prove properties

```sh
cp Receipts.cfg Empty.cfg
echo 'INVARIANT Empty' >> Empty.cfg
dive check Receipts.tla --strategy symbolic --config Receipts.cfg \
  --recheck Receipts.tla Empty.cfg --trace counterexample.json
```

The second row reports `reused: true` and a two-state counterexample to
`Empty`. Each edit binds its current modules and configuration again. The
checker compares the variable layout, concrete initial state, symmetry group
and guarded Boolean updates, including duplicate action witnesses. When all
match a previous completed search, it checks the edited obligations on the
saved reachable set. Ordinary symbolic search starts over when the relation changes. Refined sessions can also prove that an edited relation preserves reachable destinations and distances, as described below. Each violation
has a concrete trace that Dive independently validates. With `--trace`, the first violation writes the requested file and later violations include their own trace in the JSON row. Without `--trace`,
the row contains the trace.

```sh
"$PY" scripts/symbolic/check.py Receipts.tla Receipts.cfg --prove-inductive
"$PY" scripts/symbolic/check.py Receipts.tla Receipts.cfg --write-predicate reached.json
"$PY" scripts/symbolic/check.py Receipts.tla Receipts.cfg --verify-predicate reached.json
```

`--prove-inductive` tries the conjunction of the configured invariants.
`--verify-predicate` reads a candidate Boolean predicate from any prior run.
The receiver proves that its own Init implies the candidate, every action
preserves it, and it implies every configured safety obligation, including
requested deadlock freedom. An inductive proof can succeed even after a
transition edit. A failed proof obligation is inconclusive. Run a search for
an actual counterexample.

## Admission and reports

The configured layout invariant, named `TypeInvariant` by default, supplies
one finite bound per state variable. Select another with `--type-invariant`.
Finite scalar fields can contain integers, strings, untyped model values and records over supported scalars. Subsets and fixed-domain functions can use those finite domains. Ground assumptions must evaluate to TRUE. Boolean configuration flags can select constant branches. Each enabled Init witness must give a ground, complete assignment. Next can use bounded choices, disjunction,
`UNCHANGED` and deterministic assignments to all variables per action
witness. A primed value must be assigned before it is read. The expression
subset includes Boolean guards, equality, membership, set union,
intersection, difference, bounded quantifiers, finite function constructors
and single-clause `EXCEPT` through known indices. In action position each
disjunction arm and bounded existential witness must assign at least one
variable. Terminal action guards must be Boolean literals, negation,
equality, inequality, membership, nonmembership or subset tests. These
restrictions preserve TLC's generated count when assignment order or
operator evaluation would change the multiplicity of a Boolean guard.
Conditionals in assigned values and quantifiers in invariants are supported.
The checker verifies that assignments fit the proposed layout. A bound never silently removes an
initial state or successor.

Use `INIT` and `NEXT`, or a safety `SPECIFICATION` of the form
`Init /\ [][Next]_vars` covering every variable. The checker can reduce a
full permutation group of up to six model values that occur as field indices
or set members. It checks invariance of Init, each state constraint, each property and the complete
multiset of guarded updates. Record-valued domains with symmetry, partial groups, temporal properties, fairness,
action constraints, VIEW, stateful operations, substitutions and expressions
outside the implemented fragment give an explicit refusal.

A CFG `CONSTRAINT` limits which states enter the search queue. Every Init witness and every successor generated from an admitted state contributes to the generated count. The checker tests invariants even on emissions that fail the constraint. An excluded emission can end a validated counterexample. Distinct counts include only admitted states, and deadlock checking and successor generation use only admitted sources. A finite-set bound such as `Cardinality(network) <= MaxNetwork` uses those same rules.

A saved predicate also covers excluded emissions so a later invariant must hold on them. The receiver rechecks Init, the obligations on the whole candidate, and closure under steps from sources satisfying its current constraint. Widening the constraint can invalidate a previous proof.

The tool emits one JSON line per check. Search and proof rows identify their
module and configuration and use format `dive-symbolic-report-v1`. A
successful search reports exact distinct and generated counts and breadth-first
depth. A counterexample reports a shortest failing depth and the validated
ending. At a given depth multiple violations can be reachable, so the chosen
ending and witness can differ from Core's discovery order. A violation
carries no explicit-search prefix counts. A proof row reports
`proof: inductive-safety` or `proof: inconclusive` and carries no
exploration counts. See [symbolic evidence and TLC](../tlc-differences.md#d-030-finite-symbolic-safety-evidence).

| Exit | Meaning |
| --- | --- |
| 0 | Each requested check completed successfully or proved induction. |
| 1 | A requested check produced an independently validated violation. |
| 2 | Unsupported input, inconclusive induction, or command usage error. |
| 3 | Internal checker error. |
| 4 | Interrupted worker or exhausted resource budget. |

Unproved, refused and interrupted checks establish no safety result.
`--time-limit SECONDS` bounds the entire invocation, including child
processes and rechecks. `--max-memory MIB` limits virtual address space
per worker process on Linux. Set an external cgroup limit when the budget
covers aggregate physical memory. Compilation admits up to 512 state bits,
16,384 visited layout fields and 100,000 expanded action witnesses. Above 52 state bits, search counts assignments with arbitrary-precision
integers over the decision diagram. JSON counters are exact decimal integers. The transferred predicate format admits 100,000 nodes and
16 MiB. Actual time and memory depend on intermediate decision diagrams.

For source and host identities, same-host TLC comparisons, growth results and
the costs of editing an invariant, see [large-model measurements](../developer/large-models.md).

### Learning which members a wide set needs

```sh
dive check --strategy symbolic PaxosLeaseChecked.tla --config PaxosLeaseBase.cfg \
  --type-invariant TypeOK --refine-domains --time-limit 15m --max-memory 8g
```

Use `--refine-domains` when a top-level set has a wide finite element type. For a declared alphabet above 256 elements the checker starts with no member bits. A missing member in Init or in an emission from a reachable admitted source causes a restart with more bits. Each pass checks the original source configuration. The final pass must discharge every reachable emission before reporting success. An emission outside the declared layout gives a named refusal. Excluded successors still undergo invariant checking and can produce validated counterexamples.

Pass `--recheck MODULE CONFIG` once per revision, in order. After a successful refined search, the next revision starts with the previous set coordinates that belong to its own declared domain. It rebinds Init, actions, constraints and invariants on every revision. A completed refined check retains its reachable set. For a transition edit with the same layout, initial states, constraint and domain checks, the worker compares the guards of each changed assignment on the previously reachable states. If every added or removed destination equals its source there, all shortest paths and the reachable set still hold. Duplicate witnesses can also change while preserving the destinations. The checker updates the generated count for the edited Init and actions and checks deadlock freedom when enabled. New invariants must hold on every initial state and emission, including those excluded by the constraint. A successful proof reports `reused: true`. An edit that changes an admitted source’s non-stuttering destinations, or otherwise falls outside this proof, gets a fresh search with surviving coordinates. A false obligation gets a fresh search for the shortest validated witness. These searches report `reused: false`. After a violation the next revision starts cold. Each counterexample is independently validated against its own revision.

```sh
dive check PaxosLeaseChecked.tla --strategy symbolic --config PaxosLeaseBase.cfg \
  --type-invariant TypeOK --refine-domains \
  --recheck PaxosLeaseChecked.tla PaxosLeaseQuarantineEqualsProposer.cfg \
  --time-limit 30m --max-memory 6g --output json
```

The checker emits a JSON row for each revision it checks. A refusal, error or exhausted budget ends the session. `refinement_rounds` counts binding and search attempts for the revision, `learned_members` counts coordinates in the final refined set layout, and `initial_hint_members` counts surviving coordinates supplied by the preceding revision. Stale coordinates drop out. If a warm layout fails or exceeds its bounds, the worker can restart cold within the original time budget. The 512-bit limit applies to every layout. Predicate transfer uses the ordinary symbolic strategy. Refining a wide domain with a configured symmetry group currently refuses. For modest explicit graphs, ordinary Core can be faster even when the refined symbolic search completes.

### Finite initial choices

An Init predicate can assign a scalar from a finite atom set, use a bounded
existential choice, or have several finite assignment branches. Each enabled
branch must assign the complete state to ground values. The checker collects
all initial states before breadth-first search and checks every initial state
when verifying an inductive predicate. Generated counts include duplicate
initial witnesses before symmetry reduction. The compiler currently bounds
explicit expansion at 100,000 initial witnesses. An empty initial set refuses.
