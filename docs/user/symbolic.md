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

## Symbolic layout for nested tables

A table such as `snapshotStore[t][k]` can describe the same transaction and key as `written[t]` and `missed[t]`. When multiple labeled tables share both complete finite index domains, the worker puts related fields next to one another. Ordinary searches keep the common inner key first. With a singleton inner domain, the bits for one outer index can sit beside all its fields. During direct induction, a larger outer domain places each transaction's cross-key obligations together. Searches and saved-predicate verification choose the same encoding. The worker rechecks all proof obligations when reading an older predicate. Configured symmetry and refined set layouts keep their own allocation rules.

| Labeled KVS experiment | Previous layout | Shared indices | Observable result |
|---|---:|---:|---|
| 2 keys, 3 transactions, complete search | 2.65 s | 1.60–1.65 s | 3,409,605 distinct, depth 23 |
| 3 keys, 3 transactions, complete search | 57.50–58.14 s | 36.18–36.40 s | 3,557,499,075 distinct, depth 30 |
| 4 keys, 3 transactions, complete search | 536.59 s | 307.51 s | 3,552,493,680,513 distinct, depth 37 |
| 4 keys, 7 transactions, direct induction | Stopped at 50 s | 6.36–6.96 s | Inductive safety proof |

All three completed searches also agreed on generated counts. On four keys the final reachable diagram dropped from 4,171 to 3,226 nodes. For the stronger model invariant `FullHasFresh`, increasing the domain to **4 keys and 24 transactions** took 109.69 s to prove. **2 keys and 56 transactions** took 10.27 s and **1 key and 102 transactions** took 7.11 s. Additional slots reached the 512-bit admission limit at 2 keys/57 slots and 1 key/103 slots. These are induction results on the source program. The independent KVS history formula gives about 5.0024 × 10¹⁰⁶ labeled reachable states at 102 slots. The proof checks initial inclusion and next-step closure without traversing these states. In the 102-slot model the weaker invariants alone failed the next-step obligation. The [measurements, domain setup and reproduction commands](../../conformance/reports/perf/2026-11-21-symbolic-table-orders.json) include both bit orders and the negative controls.

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
saved reachable set.

When an edit changes an action but preserves the encoding, initial states,
constraint and symmetry, ordinary symbolic search can reuse breadth-first
layers. For an edit that only adds transitions, every old state remains reachable
by its previous depth. The checker uses the old next layer, applies the added
actions, and expands states that first arrive at a different depth or were
unreachable before. This preserves shortest witnesses even after a shortcut.
If an edit removes an edge that could have supplied a shortest path, the
checker computes that layer again. It checks all edited obligations, including
invariants on excluded emissions, and counts the edited action witnesses.

A positive `reused_layer_images` gives the number of old layer images used
during the edit. A reused layer can still require expansion of newly reached
states. `reused: true` continues to describe reuse of the complete
previous search under an identical relation. A change to the encoding,
initial-state set, constraint or symmetry starts a fresh search. Refined
set-domain checking has its own reuse conditions below. Each violation
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
and single-clause `EXCEPT` through known indices. Each bounded existential
action witness and each action-level disjunction arm must assign a variable.
An inline state-level disjunction directly inside an action conjunction can
act as a guard. While successor variables remain unassigned, each true arm
contributes a separate witness. For example, with `Init == x = 0`,
`Next == (x = 0 \/ x = 0) /\ x' = x` generates three states including
Init, all with the same value. The checker refuses overlapping guards in
contexts where it cannot establish the count, including a disjunction reached
through a lazy Boolean parameter or after the final assignment.

Terminal guards support Boolean literals, negation, equality, inequality,
membership, nonmembership and subset tests. Conditionals in assigned values
and quantifiers in invariants are supported.
The checker verifies that assignments fit the proposed layout. A bound never silently removes an
initial state or successor.

Use `INIT` and `NEXT`, or a safety `SPECIFICATION` of the form
`Init /\ [][Next]_vars` covering every variable. The checker supports finite permutation groups over up to six distinct model values, including separate proposer and acceptor groups. Values can occur in function indices, subsets, scalar fields and records. It first searches labeled states and then counts reachable symmetry classes and the generated witnesses per representative. Every enabled Init witness contributes before symmetry reduction.

For record fields and compound groups, the checker verifies closure of the concrete finite domains, including functions with no varying output bits. It also verifies that each generator preserves Init, the constraint, the invariants and the full multiset of guarded updates. If it cannot establish equivariance for the encoded update it reports an explicit refusal. Temporal properties, fairness,
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

When the checker computes a fresh breadth-first image, it compares the decision diagrams for the admitted frontier and all admitted states reached so far. It can expand the smaller diagram because successors of earlier layers have already been visited. Subtracting the reached states yields the same next layer, preserving shortest witness depths. This choice runs automatically during ordinary symbolic exploration.

### Learning which members a wide set needs

```sh
dive check --strategy symbolic PaxosLeaseChecked.tla --config PaxosLeaseBase.cfg \
  --type-invariant TypeOK --refine-domains --time-limit 15m --max-memory 8g
```

Use `--refine-domains` when a top-level set has a wide finite element type. For a declared alphabet above 256 elements the checker starts with no member bits. A missing member in Init or in an emission from a reachable admitted source causes a restart with more bits. Each pass checks the original source configuration. The final pass must discharge every reachable emission before reporting success. An emission outside the declared layout gives a named refusal. Excluded successors still undergo invariant checking and can produce validated counterexamples.

Pass `--recheck MODULE CONFIG` once per revision, in order. The checker binds each revision from source. It carries forward learned set coordinates that still belong to the declared domain. A batch also retains the completed breadth-first layers, including constraint-excluded states for invariant checking. A single refined check uses the lower-memory search that retains only admitted states.

When the layout, initial states, constraint and domain checks agree, an action edit has two opportunities for reuse.

- If every added or removed edge from an admitted reachable state returns to that state or to an admitted initial state, shortest paths and reachability are preserved. The checker adjusts the generated count for edits to initial or action witness multiplicities. It checks new invariants against every initial state and emission, and checks deadlock freedom when necessary. A successful proof reports `reused: true`.
- For other action edits, the checker reuses each old layer image whose removed transitions cannot eliminate a new destination. It adds all new destinations from the edited transitions. For additions alone it carries the old images through the changed layers and expands newly reached or earlier-arriving states. For removals it resumes ordinary expansion when the shortest layers differ. The report gives `reused: false` and includes `reused_layer_images` when at least one image was saved.

A change to admission or the domain checks starts a fresh search with the surviving coordinates. Every reported counterexample undergoes independent validation against its own revision. After a violation the next revision starts cold.

```sh
dive check PaxosLeaseChecked.tla --strategy symbolic --config PaxosLeaseBase.cfg \
  --type-invariant TypeOK --refine-domains \
  --recheck PaxosLeaseChecked.tla PaxosLeaseQuarantineEqualsProposer.cfg \
  --time-limit 30m --max-memory 6g --output json
```

The checker emits a JSON row for each revision it checks. A refusal, error or exhausted budget ends the session. `refinement_rounds` counts binding and search attempts for the revision, `learned_members` counts coordinates in the final refined set layout, and `initial_hint_members` counts surviving coordinates supplied by the preceding revision. Stale coordinates drop out. If a warm layout fails or exceeds its bounds, the worker can restart cold within the original time budget. The 512-bit limit applies to every layout. Predicate transfer uses the ordinary symbolic strategy. When symmetry is configured, retained set members include every image of each learned member under the group. For modest explicit graphs, ordinary Core can be faster even when the refined symbolic search completes.

### Finite initial choices

An Init predicate can assign a scalar from a finite atom set, use a bounded
existential choice, or have several finite assignment branches. Each enabled
branch must assign the complete state to ground values. The checker collects
all initial states before breadth-first search and checks every initial state
when verifying an inductive predicate. Generated counts include duplicate
initial witnesses before symmetry reduction. For an initial assignment
`x \in S`, the checker must establish that TLC enumerates each member of
`S` exactly once. Explicit sets, integer intervals and record products with
duplicate-free field enumerations meet that condition. For example, the set
`{[a |-> 0], [a |-> 0]}` produces one initial witness. TLC can
enumerate the same record twice from `[a : {0}] \cup [a : {0,1}]`. The
symbolic checker refuses this initial membership choice even through an
operator or `LET` binding. It accepts disjoint unions of proven duplicate-free
domains. The compiler bounds explicit expansion at 100,000 initial witnesses.
An empty initial set refuses.
