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
"$PY" scripts/symbolic/check.py Receipts.tla Receipts.cfg \
  --recheck Receipts.tla Empty.cfg --trace counterexample.json
```

The second row reports `reused: true` and a two-state counterexample to
`Empty`. Each edit binds its current modules and configuration again. The
checker compares the variable layout, concrete initial state, symmetry group
and guarded Boolean updates, including duplicate action witnesses. When all
match a previous completed search, it checks the edited obligations on the
saved reachable set. A changed relation runs a fresh search. Each violation
has a concrete trace that Dive independently validates. Without `--trace`,
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
Supported fields are finite strings and untyped model values from finite sets, subsets of those
sets and fixed-domain functions over them. Each enabled Init witness must give a ground, complete assignment. Next can use bounded choices, disjunction,
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
or set members. It checks invariance of Init, each property and the complete
multiset of guarded updates. Partial groups, temporal properties, fairness,
constraints, VIEW, stateful operations, substitutions and expressions
outside the implemented fragment give an explicit refusal.

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


### Finite initial choices

An Init predicate can assign a scalar from a finite atom set, use a bounded
existential choice, or have several finite assignment branches. Each enabled
branch must assign the complete state to ground values. The checker collects
all initial states before breadth-first search and checks every initial state
when verifying an inductive predicate. Generated counts include duplicate
initial witnesses before symmetry reduction. The compiler currently bounds
explicit expansion at 100,000 initial witnesses. An empty initial set refuses.
