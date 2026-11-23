# Finite symbolic safety checking

Use `dive check --strategy symbolic` in a Linux release bundle when a finite TLA+
model has many combinations of the same few fields. It can search the complete
reachable set, check a new invariant against that set, or prove safety by
induction. It reads the original TLA+ module and configuration through Dive's
binder.

## Check the AsyncRaft log invariant

The source-checkout command below checks whether a leader contains every entry
of its own term at the same index where any server has it. It reads the
unchanged Scimitar AsyncRaft module and 29 strengthening formulas from the
released proof graph. Install `z3-solver==5.4.1.0` in a Python environment and
check out the Scimitar revision pinned in the evidence below. Build the
Dive exporter with `cargo build --release -p dive-api --example symbolic_input`.

```sh
python3 scripts/perf/scale/raft_induction/check.py scimitar/benchmarks \
  --exporter target/release/examples/symbolic_input --servers 3 --save proof.json
```

Replace `--save` with `--verify` to rebind the source, independently repeat
every solver obligation and compare the receipt, including after moving the
checkout. Use `--servers 2` for the smaller carrier. The slot encoding
has terms 0–2 and at most two entries per log. Each channel holds up to two
messages by default. Add `--max-messages 3` to check three. The receiver must
use the same capacity. The checker derives the number of symbolic message
slots from the bound TLA+ constant. The checker evaluates
all 11 actions in the original `Next` and checks safety even on destinations
outside the source state constraint. The published rejection and truncation
actions are disabled in that relation.

On the three-server carrier, checking the 1,605-byte receipt took **23.31 s**
and fresh verification took **23.37 s**. Ordinary Core exhausted the same
model in **3.89 s** with 163,681 distinct states. Choose Core for a fast
verdict at these bounds. The symbolic command provides independently
recheckable, action-by-action closure of the published strengthening.

Changing only `MaxMsgCount` from two to three reverses the measured outcome.
The configurable checker finished in **38.17 s** at 82 MiB, and a relocated
process verified the 1,607-byte receipt in **38.32 s**. A second dedicated
Linux host reproduced the proof in **40.46 s**. At a 65-second limit on that
host, exhaustive Core had generated **19,006,912**
states, found **3,743,621** distinct states and still had **1,228,744** queued.
It returned a resource stop. At capacity two, Core on that same host finished
in 3.63 s. The three-slot proof covers 48 feasible alternatives. Eighteen can
leave the constraint.

The integrated panel also replays a real leader election, deliveries of both
log indices to another server and a subsequent term change. It rejects a
changed lemma, stale proof, missing action and damaged word. Allowing a
follower to append a client entry produces an independently validated
seven-state counterexample. The first bad entry is at index 1 in term 1.
A second mutant enables the follower only when three distinct vote requests
are pending. Its strengthening holds at capacity two. At capacity three an
eight-state word reaches its first failure at index 1.
On the original module, a repeated send has cardinality one, and another
execution step consumes one of three distinct requests.
[Source, commands, peer results and measurements](../../conformance/reports/perf/2026-11-22-asyncraft-log-induction.json).

### Use the published term, log and channel bounds

For three servers, terms through 3, logs of length 3 and 1,000 messages per
channel, pass `--message-sets`. The checker represents each channel by a
membership function on complete wire records. Checking all 30 predicates
covers 28 guarded action alternatives. Increasing the configured channel
capacity adds no Boolean slots.

On a dedicated c7i.4xlarge, six action workers proved the published bounds in
**18.19 s** and a relocated process verified the 2,438-byte receipt in
**18.33 s**. One worker verified the same receipt in **52.67 s**. Six-worker
Core found 8,878,296 distinct states and still had 5,989,499 queued at its
30-second cutoff. Both runs checked the same log invariant on the same CFG.

The Scimitar CFG uses these exact bounds and names a different invariant.
Select the published proof graph’s log invariant and retain its constants:

```sh
sed 's/^INVARIANT H_CandidatesOrLeadersVotedForSelf$/INVARIANT H_PrimaryHasEntriesItCreated/' \
  scimitar/benchmarks/AsyncRaft.cfg > RaftLog.cfg
printf '\nCHECK_DEADLOCK FALSE\n' >> RaftLog.cfg
python3 scripts/perf/scale/raft_induction/check.py scimitar/benchmarks \
  --exporter target/release/examples/symbolic_input --config RaftLog.cfg \
  --message-sets --jobs 6 --save published-log.json
```

To verify the saved receipt, use the same inputs and replace `--save` with
`--verify`. The verifier can use a different number of worker processes.
Each receipt binds the supplied CFG bytes, resolved TLA+ and strengthening
formulas, named target, bounds, encoding and action coverage. The output also
reports solver formula size and query counts. A timeout, unsupported expression
or undischarged obligation returns `inconclusive`. A reachable violation
requires a separately validated original execution.

For terms through 2 and logs of length 2 at capacity 1,000, omit `--config`
and pass `--servers 3 --max-term 2 --max-log-length 2 --max-messages 1000`.
The same membership proof checks all generated destinations for safety,
including those outside the configured state constraint. Each retained
destination must also satisfy every strengthening and the typed representation.
[Measurements, peer controls and reproduction commands](../../conformance/reports/perf/2026-11-22-asyncraft-channel-sets.json).

## Choose a strategy automatically

Run `dive check Model.tla --strategy auto --workers 8 --output json` to check
the original configuration. The selector considers all configured invariants
and deadlock checking. A temporal property goes to ordinary Core. A named
proof, a bounded bug search and an edit session each have a different
completion condition and use their own commands.

On Linux with the matching companion, the selector binds the original
source. It tries exact population counts when the actions and invariants
treat the processes in a status table interchangeably. Otherwise it derives a
finite bit layout from a configured invariant. A 22–32 bit layout with an
action that changes several coordinates can use the packed compiler when its
visited bitmap fits a quarter of the memory budget and the installed compiler
and relation pass admission. Independent single-bit updates, wider layouts
and absent compilers use decision diagrams when admitted. These thresholds
come from the measured workloads. They predict cost and have no role in
establishing a verdict. [Measurements and controls](../../conformance/reports/perf/2026-11-22-auto-strategy.json).

For these requests, workers (1–8) cap parallel packed search. Population and
decision-diagram searches use one worker. A finite result has
`format: "dive-symbolic-report-v1"`, the selected `strategy`, and the
[finite strategy exit codes](#admission-and-reports). Core handles sources
outside the finite fragment and binary-only installations. It returns its
ordinary JSON report and exit code, with exact state identity for this
request. The companion independently replays a concrete Core counterexample
before forwarding it. The wall budget covers selection, search and replay.
The finite path uses half of physical RAM as its default per-process address
space limit. Explicit time and memory flags also apply.

Core-specific options such as `--max-states`, `--compat=tlc`,
`--output human` and `--workers auto` go straight to Core with the supplied
options, report format and exit convention. The same applies when a Core
compatibility or diagnostic environment override is set. Core announces
selection on stderr. Programs consuming both report types can inspect the
finite `format` field or Core's `schema_version`.

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

## Check finite records and sets as whole values

Some specifications declare a message as a record and update the entire message at once. For an eligible fresh safety check, the symbolic worker assigns a small integer code to each possible value of each variable. A decision diagram can then share the same message and policy choices across many combinations of states. The report identifies this encoding with `"encoding": "whole-values"`.

Run the ordinary symbolic command on the current source. For example, with the [released EmergencyCall module and f3 configuration](https://github.com/FormalCellular/EmergencyCall/tree/093684f1bb02d733cae95d50f1690f1cb792296a/model):

```sh
dive check EmergencyCall_Model_auto.tla --config config/f3.cfg --strategy symbolic --output json
```

This check starts with 49,152 states and reaches 21,255,936 distinct states in 27 levels. The 135,871,232 generated occurrences include initial states and each enabled action witness, including actions that reach the same destination. To save an independently validated counterexample on a failing configuration, add `--trace witness.json`.

The selected type invariant must appear in the configuration and as a required conjunct of the initializer. This encoding accepts finite whole records, sets, strings and integers. Each initial path must bind every variable before reading it, and initial choices must give a provably unique witness per state. A variable omitted from the type predicate needs a mandatory literal initial assignment and a finite inferred alphabet. Every reachable enabled assignment must be defined and stay within its alphabet. Ambiguous initial counts, incomplete actions and unsupported expressions give a named refusal. Whole-value admission covers INIT/NEXT safety checks without CFG constants, symmetry, constraints or operator overrides. In a sequence of `--recheck MODULE CFG` arguments, a completed search keeps the reachable set in memory. If only the configured invariant list changes, dive checks every selected invariant against that set and reports `reused: true` when they all hold. The initial, distinct and generated counts and breadth-first depth then apply to the edited check. A failing predicate triggers fresh breadth-first search and independent trace validation. A change to the bound model, finite layout or deadlock setting also triggers a fresh check. [Full configuration and independent controls](../developer/large-models.md#check-the-full-emergencycall-initial-predicate).

On EmergencyCall with three extra local numbers, the final worker checked both invariants from scratch in 1,038.55 s and reached **1,400,862,255,552 distinct states**. Checking the type invariant alone in the same process took 118 ms. Restoring both invariants took 43 ms. All three revisions reported the same initial, generated and reachable counts and depth. On the original full configuration, adding the violated `F1` invariant produced an independently validated seven-state counterexample. [Commands, timings, memory and replay controls](../../conformance/reports/perf/2026-11-21-whole-value-edit-sessions.json).


### Reuse a whole-value predicate in another process

A completed check can save its finite reachable predicate with `--write-predicate FILE`.
On the current source and configuration, a separate installation can submit it with
`--verify-predicate FILE`. The receiver checks the whole-value layout, inclusion of
every initial state, defined successor expressions, closure under every original
action, safety and any requested deadlock check. A successful verification returns
`inductive-safety` without search counts. When both processes have the same local
`DIVE_SYMBOLIC_PROOF_KEY`, an authenticated predicate can reuse the transition
obligations for the matching bound relation and checks all current invariants.

Sail's `runtime symbolic-check` session can use the same artifact across processes.
If the predicate cannot prove the current check, Sail searches with the time left
on the original deadline. On the original full-initializer EmergencyCall model,
restoring the safety obligation took 104.539 s cold, 1.928 s with an unsigned
predicate, and 0.676 s through a separately launched Sail process with local
authentication. [Matched runs and failure controls](../../conformance/reports/perf/2026-11-22-whole-value-portable.json).

## Transfer a complete finite induction certificate

On Linux, use `dive check MODULE --config CFG --strategy finite-induction --output json
--write-predicate FILE` on an admitted finite safety configuration. The
checker explores original Init and Next with exact values in the Dive profile.
The certificate
holds the finite predicate as value dictionaries and a set of encoded rows.
The checker conservatively refuses representation-sensitive operators in
the loaded modules, including some imports that expose those operators.
The verifier checks safety on each generated successor, including states
outside a configured state constraint. Only in-domain rows are expanded.

A separate process can load the certificate with `--verify-predicate FILE`.
Without a local signing key, it independently checks the original initial
states, every bound action, closure and the configured invariants. When
`DIVE_SYMBOLIC_PROOF_KEY` points to the same qualifying local key in both
processes, Dive can reuse Init and action closure on the same bound model.
It checks every current invariant over the admitted rows and the recorded
boundary successors. Changing an imported action, the carrier or the
constraint changes the binding. A successful verification returns
`inductive-safety` with checked and reused obligations.

To transfer a candidate from an independent finite proof, pass
`--verify-predicate INPUT --write-predicate OUTPUT` in the same invocation.
Dive rechecks the supplied rows, regenerates the boundary evidence, and
records the current proved formulas before it writes the new certificate.
The TwoPhase and AsyncRaft proof drivers can save their certified candidates
with `--certificate-out DIR`.

An installed Sail client can use the same strategy with
`sail runtime symbolic-check --strategy finite-induction`. It searches the
current configuration within the remaining session deadline if a candidate
is inconclusive. [Complete TwoPhase and AsyncRaft edit histories](../../conformance/reports/perf/2026-11-22-portable-finite-induction.json).

## Compile a finite safety check

Use `dive check --strategy compiled` in the Linux release bundle to compile a
small finite state layout into native guards and updates. Install `rustc`
1.99.0 and the host linker before running this strategy. Dive resolves an
installed compiler through `PATH` or rustup and compiles offline.

```sh
dive check MCKVS.tla --config MCKVSSafetyMedium.cfg --strategy compiled \
  --workers 8 --max-memory 4g --time-limit 30s --output json
```

Compiled checking uses the finite Boolean source admission. It accepts layouts
up to 32 bits directly. Layouts from 33 through 63 bits require proved index
symmetry with two to six interchangeable lanes, each narrower than 16 bits.
The unswapped part must also be narrower than 16 bits. The wide checker ranks
sorted lane values using the projections of the configured invariants and
admits a membership bitset of up to 8 GiB. The layout comes from
`TypeInvariant`, or the configured invariant selected with
`--type-invariant NAME`. The checker requires a complete `Init` and `Next`,
supports configured safety invariants and deadlock checking, and refuses
state constraints, liveness and expressions outside the supported fragment.
Every invocation binds and compiles the current source and configuration.

The worker searches breadth-first using exact keys. It counts each enabled
successor occurrence before deduplication and reports states modulo configured
symmetry. `--workers` accepts 1 through 8 and defaults to 1. The JSON report
gives the largest width used, `state_bits`, the membership bitset capacity in
bits, `membership_bits`, compilation and search times, and complete counts
on success. A successor outside the projected rank space still undergoes
invariant checking. When the first failing layer identifies a single
invariant, the compiled worker repeats the search to that layer and retains
concrete predecessors for a shortest trace. If the search finds a deadlock
or competing invariant names, exact Core finds the trace. Dive independently
validates either trace. The report gives the search used as
`witness_strategy` and the time through replay as `witness_seconds`.
`--trace FILE` saves the validated trace. The time limit covers binding,
compilation, search, and replay. The memory limit applies to each process's
address space. This strategy uses the finite checker exit codes above.

With an injected depth-13 invariant failure in KVS Large, the eight-worker
compiled command returned a validated trace in 2.75 s. The same command had
taken 30.73 s with the Core rerun. One and eight workers returned identical
traces. [Inputs, timings and holding controls](../../conformance/reports/perf/2026-11-21-compiled-witness.json).

## Checking interchangeable status tables

Use `dive check --strategy occupancy` when a finite population shares one table of local string statuses. This optional Linux strategy reads the bound TLA+ and configuration and performs exact, labeled breadth-first search. Run the three-manager control from the published Examples `specifications/transaction_commit` directory.

```sh
dive check TCommit.tla --config TCommit.cfg --strategy occupancy \
  --type-invariant TCTypeOK --time-limit 30s --max-memory 512m --output json
```

The released three-manager configuration returns 34 concrete distinct states, 94 generated occurrences, depth 7 and 13 reachable population vectors. A vector records the number of participants in each local status. The checker explores one vector per orbit, weights it by the number of labeled assignments, and counts every enabled original action witness, including duplicate and self-loop witnesses. It stores an exact integer key for each vector and the participant move that discovered it. On a violation it reconstructs a shortest path with concrete identifiers and runs `dive validate-trace` before returning exit code 1. Use `--trace FILE` to save that validated word.

Admission currently requires one total table whose declared type is `[Ids -> {"s₁", …, "sₖ"}]`, a finite configured set of distinct untyped model-value participants and at most eight string statuses. Each initial alternative assigns a uniform status to the whole table. Each action branch stutters or updates a single existentially bound participant to a literal status. Guards can read that participant's status and quantify over statuses present in the complete participant domain. Invariants can quantify over one or two participants, including coincident indices. Input that distinguishes a named participant, correlates a second table, constrains admitted states, selects CFG symmetry or uses another unsupported feature returns a named refusal. The ordinary Core and symbolic strategies remain available for richer sources.

`--max-orbits N` bounds the number of reachable vectors retained, including initial vectors. The default is 1,000,000. Set a larger limit explicitly for larger exact searches. Time, memory and orbit-budget stops carry no completed counts. Success rows include integer `distinct` and `generated`, lossless decimal strings `distinct_decimal` and `generated_decimal`, `depth`, `occupancies` and the configured invariant names. A counted search starts at Init and admits up to 8,192 participants. This bounds the size of its exact integer reports.

For a source checkout, use the same supervisor with `python3 scripts/symbolic/check.py MODEL.tla MODEL.cfg --occupancy`, supplying `--dive`, `--exporter` and `--type-invariant` as needed. [Source admission, measurement data and reproduction](../developer/large-models.md#checking-anonymous-processes-by-population).

### Transfer a population safety predicate across edits

A completed exact population search can write a separate candidate file. The receiving checker binds the entire current source and CFG, checks that they still admit the population reduction, and verifies initial inclusion, all current transitions and every configured invariant or deadlock check. The candidate describes which **sets of present statuses** are allowed. Its labels can be reordered in the receiving source, and the number and names of participants can change.

```sh
dive check TCommit.tla --config TCommit.cfg --strategy occupancy --type-invariant TCTypeOK \
  --write-predicate support.json --output json
dive check Edits.tla --config stronger.cfg --strategy occupancy --type-invariant TCTypeOK \
  --verify-predicate support.json --output json
```

A successful receiver reports `proof="inductive-safety"` without counts or depth. A failed obligation gives `proof="inconclusive"` and names the obligation. An abstract bad state may be unreachable. Run a fresh exact search for labeled counts or a concrete, independently validated counterexample. Use `--prove-inductive` for a fresh support proof without a supplied file. The proof binds every configured participant and checks its identity. Larger finite domains are limited by frontend resources. Induction requires at most 256 sets of present statuses and can be as fast as verifying the stored predicate. The file is a portable candidate, and its contents confer no authority before verification. [Proof obligations, false controls and reproduction](../developer/large-models.md#recheck-population-predicates).

## Symbolic layout for nested tables

A table such as `snapshotStore[t][k]` can describe the same transaction and key as `written[t]` and `missed[t]`. When multiple labeled tables share both complete finite index domains, the worker puts related fields next to one another. Ordinary searches keep the common inner key first. With a singleton inner domain, the bits for one outer index can sit beside all its fields. During direct induction, a larger outer domain places each transaction's cross-key obligations together. Searches and saved-predicate verification choose the same encoding. The worker rechecks all proof obligations when reading an older predicate. Configured symmetry and refined set layouts keep their own allocation rules.

| Labeled KVS experiment | Previous layout | Shared indices | Observable result |
|---|---:|---:|---|
| 2 keys, 3 transactions, complete search | 2.65 s | 1.60–1.65 s | 3,409,605 distinct, depth 23 |
| 3 keys, 3 transactions, complete search | 57.50–58.14 s | 36.18–36.40 s | 3,557,499,075 distinct, depth 30 |
| 4 keys, 3 transactions, complete search | 536.59 s | 307.51 s | 3,552,493,680,513 distinct, depth 37 |
| 5 keys, 3 transactions, complete search | Unmeasured | 1,537.37 s | 3,428,937,742,115,475 distinct, depth 44 |
| 6 keys, 3 transactions, complete search | 10,696.03 s | 4,712.57 s | 3,239,447,207,251,422,105 distinct, depth 51 |
| 4 keys, 7 transactions, direct induction | Stopped at 50 s | 6.36–6.96 s | Inductive safety proof |

Each paired complete search also agreed on generated counts. The labeled five-key search agreed with the independent history calculation on both counts. On four keys the final reachable diagram dropped from 4,171 to 3,226 nodes. On six keys it fell from 7,099 to 5,756 nodes, and peak RSS fell from 1,294,016 to 1,148,068 KiB. Both six-key layouts reported 130,350,884,965,021,924,615 generated occurrences. For the stronger model invariant `FullHasFresh`, increasing the domain to **4 keys and 24 transactions** took 109.69 s to prove. **2 keys and 56 transactions** took 10.27 s and **1 key and 102 transactions** took 7.11 s. In that comparison, additional slots reached the previous 512-bit direct-proof limit at 2 keys/57 slots and 1 key/103 slots. These are induction results on the source program. The independent KVS history formula gives about 5.0024 × 10¹⁰⁶ labeled reachable states at 102 slots. The proof checks initial inclusion and next-step closure without traversing these states. In the 102-slot model the weaker invariants alone failed the next-step obligation. The [measurements, domain setup and reproduction commands](../../conformance/reports/perf/2026-11-21-symbolic-table-orders.json) include both bit orders and the negative controls.

### Direct induction on wider finite models

Use `--prove-inductive` to check whether the configured invariants hold initially and are preserved by every permitted transition. Ordinary finite layouts and index-only symmetry admit up to **2,048 state bits** for this operation. Complete exploration, verification of a saved predicate, domain refinement and symmetry that permutes stored values retain their 512-bit bounds. A failed next-step obligation can involve an unreachable state. Use search to produce a reachable counterexample.

The bound alone does not explain the improvement on Lamport's transaction commit model. Its consistency property forbids an aborted manager and a committed manager in the same state. A Cartesian expansion examines one million index pairs at 1,000 managers. The worker can check this condition by finding whether each of the two kinds of manager exists. This takes two scans of the same finite domain and includes the case where the indices coincide. This rule applies to two independently indexed unprimed finite tables compared to string or integer literals. Dependent tests, tuple binders and partial tables use the general quantifier evaluator.

With the 1,000 resource managers listed in `RM`, the original `TCTypeOK` and `TCConsistent` proved in **99.01 s** with **584.4 MiB** peak RSS. The public module needed no source edit or additional invariant. For KVS with two values the stronger `FullHasFresh` configuration also proved at **200 keys and two symmetric transactions in 56.22 s**, and at **one key and 256 labeled transactions in 75.05 s**. The independent KVS history formula gives approximately 6.859 × 10⁴¹⁵ reachable transaction-symmetry classes for the 200-key model and 1.185 × 10²⁶⁷ labeled states for the 256-transaction model. Each direct proof reports only the safety obligations for its configured finite domain. The [data and reproduction commands](../../conformance/reports/perf/2026-11-21-wide-source-induction.json) include the 2,048-bit boundary, negative controls, and runs isolating the two changes.

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
If removing an action could eliminate a shortest path, the checker looks for
a set of states that contains all removed destinations and that every original
transition stays inside once entered. It can reuse the old shortest layers
outside that set. Inside, it expands the surviving actions and includes paths
entering from outside. If the proof finds no useful region, ordinary
breadth-first expansion resumes. It checks all edited obligations, including
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

The [KVS source-edit example](../developer/large-models.md#recheck-a-kvs-invariant-after-an-edit-to-the-protocol) shows an installed Sail session proving an invariant after a protocol change, finding a real violation after an unsafe edit, and using a strengthening to check larger domains.

## Admission and reports

The configured layout invariant, named `TypeInvariant` by default, supplies
one finite bound per state variable. Select another with `--type-invariant`.

Source binding skips pure closed definitions that no configuration role,
assumption or observable startup computation can reach. A library can define
`Permutations(Peer)` for other configurations without constructing that set
when the selected configuration never uses it. Selecting the definition under
`SYMMETRY` still evaluates it and applies the normal symmetry limits.
[Bounded export and checker controls](../../conformance/reports/perf/2026-11-21-symbolic-source-binding.json).

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
covers aggregate physical memory. Compilation admits 16,384 visited layout fields and 100,000 expanded action witnesses. The ordinary search limit is 512 state bits. Eligible direct proofs admit 2,048 bits. Above 52 state bits, search counts assignments with arbitrary-precision
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
- For other action edits, the checker reuses each old layer image whose removed transitions cannot eliminate a new destination. It adds all new destinations from the edited transitions. For additions alone it carries the old images through the changed layers and expands newly reached or earlier-arriving states. For deletions alone it may reuse the part of each later layer outside a region proved closed under the original actions. The report gives `reused: false` and includes `reused_layer_images` when at least one image was saved.

A change to admission or the domain checks starts a fresh search with the surviving coordinates. Every reported counterexample undergoes independent validation against its own revision. After a violation the next revision starts cold.

```sh
dive check PaxosLeaseChecked.tla --strategy symbolic --config PaxosLeaseBase.cfg \
  --type-invariant TypeOK --refine-domains \
  --recheck PaxosLeaseChecked.tla PaxosLeaseQuarantineEqualsProposer.cfg \
  --time-limit 30m --max-memory 6g --output json
```

The checker emits a JSON row for each revision it checks. A refusal, error or exhausted budget ends the session. `refinement_rounds` counts binding and search attempts for the revision, `learned_members` counts coordinates in the final refined set layout, and `initial_hint_members` counts surviving coordinates supplied by the preceding revision. Stale coordinates drop out. If a warm layout fails or exceeds its bounds, the worker can restart cold within the original time budget. The 512-bit limit applies to every refined layout. Predicate transfer uses the ordinary symbolic strategy. When symmetry is configured, retained set members include every image of each learned member under the group. For modest explicit graphs, ordinary Core can be faster even when the refined symbolic search completes.

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

## Prove one configured state invariant

Use `--property-proof NAME` with the symbolic strategy when the question is
whether a particular invariant holds. For example, the original Recomp-Verify
lock server configures `Mutex` and its module defines a finite layout called
`TypeOK`.

```sh
dive check lockserv.tla --strategy symbolic --property-proof Mutex --type-invariant TypeOK
dive check lockserv.tla --strategy symbolic --property-proof Mutex --type-invariant TypeOK \
  --write-predicate mutex.json
dive check edited/lockserv.tla --config lockserv.cfg --strategy symbolic \
  --property-proof Mutex --type-invariant TypeOK --verify-predicate mutex.json
```

A named proof can use a finite relation such as `sent \in SUBSET (Clients \X Requests)`,
tuple literals and tuple-pattern quantifiers over it. An initial choice or binder
over `SUBSET S` represents each subset exactly when every member occurs in the
current `S`. The named worker accepts at most 12 possible members for that
enumeration and 16,384 combinations per tuple expression. Exceeding a bound
gives an explicit refusal. A subset used directly as a tuple component must
be ground. [Original Recomp-Verify configurations and measured limits](../../conformance/reports/perf/2026-11-22-recomp-originals.json)

For a finite function choice in Init, such as `table \in [Node -> SUBSET Key]`,
the checker enumerates each ground function and applies the original Init
guards. Each choice has a limit of 65,536 functions, 1,024 keys and, for
multi-valued ranges, 16 keys. The worker checks state-level quantified guards
and can compare a Boolean-valued function cell with a Boolean expression.
It treats equivalent successors as one possibility in a named proof. A finite
integer can be tested for membership in `Nat` or `Int`; modulo requires a
positive divisor. Enumerating an unbounded integer set still refuses.

When Init chooses a function from the layout's finite scalar function space,
the named worker can represent the whole choice with the function's state
bits. The original Init predicates still restrict the assigned values. If a
predicate requires ground evaluation, the worker uses the bounded enumerator.

The layout can be an unconfigured, parameterless source operator. Its bounds
must admit Init and the encoded assignments. A variable without a finite
declaration may be a private integer initialized to a literal. Each step must
preserve it, reset it to a literal, or add an integer literal to itself.
A read in a guard, retained assignment, constraint or property can require
that variable and cause a refusal. The checker reconstructs integer values
when it produces a witness.

If you omit `--type-invariant` and the source has no default `TypeInvariant`
operator, a named proof can infer a layout. The checker starts from the original
Init values, then grows the scalar and set domains and fixed-domain function
ranges by checking enabled assignments. It certifies that every original
guarded action stays within the layout before using it for a proof. The search
tries up to eight layouts and grows at most 256 alternatives per field.
It reports `"layout": "inferred"` when it uses a certified layout. Pass
`--type-invariant NAME` to require a particular source operator.

The `dive-property-proof-v1` report names the property. `inductive-safety`
means the checker established initial inclusion, preservation by every guarded
action and implication of the requested invariant. A fresh proof first tries the
requested invariant itself. It checks preservation by substituting the action's
simultaneous updates into the predicate under the original guard. If that
candidate fails, the checker searches for a closed predicate or a concrete
counterexample. Other configured invariants,
deadlock and temporal properties require separate checks. Graph counts and
diameter require a reachability run. A violation includes an independently
validated concrete trace. A transferred predicate that fails induction yields
`inconclusive` and names the failed obligation.

`support_variables` and `support_bits` describe the closed dependencies of the
property and constraints in the bound transition relation. A closure sweep
applies each action to everything reached so far. With `--recheck MODULE CFG`,
the checker binds each revision. An unchanged guarded transition can retain
its proof for the same predicate and compatible encoding. The report counts
the retained obligations. In another process `--verify-predicate` checks every
obligation again by default. [Measured comparisons and edit controls](../developer/large-models.md#prove-a-property-on-the-original-recomp-verify-models).

### Reuse certified action obligations in another process

For a series of expensive named-property checks on a trusted Linux machine,
initialize a private 32-byte key once, outside the source tree and the directory
where peers exchange predicates. Use a new path for the first command.

```sh
(umask 077 && set -C && head -c 32 /dev/urandom > "$HOME/.dive-proof-key")
export DIVE_SYMBOLIC_PROOF_KEY="$HOME/.dive-proof-key"
```

Run the write and verify commands above with this environment. Dive signs the
identities of transition obligations that passed induction. A receiver always
rebinds the original program, checks its assumptions, lowers every action,
checks Init and proves that the candidate implies the requested property. It can
skip an image only for the same candidate, finite encoding, admission,
property, checker implementation, Boolean guard and simultaneous bit updates.
Edits to imported operators take effect through these bound Boolean functions.
An equivalent edit can preserve an obligation. The proof report counts skipped
images in `reused_obligations`. A receiving process can verify an unsigned
predicate normally.

Anyone with the key can attest an obligation. Give it only to trusted checker
processes with an immutable installation. The checker opens a mode-0600 or mode-0400
regular file and ignores an unavailable key, a wrong key or an invalid tag.
Removing the environment variable disables the optimization. Rotating the key
causes old signatures to miss. On small models or cheap transition images,
hashing the implementation and Boolean functions can take longer than proving
the images again.

obligation again. [Measured comparisons and edit controls](../developer/large-models.md#prove-a-property-on-the-original-recomp-verify-models).


When many actions use one global state predicate, named checking evaluates a
parameterless Boolean definition once for the current revision when its
lexical frame and pending assignment are empty. A new revision binds its own
guard. Functional updates only encode changed Next coordinates when the other
values already refer to the corresponding current state.

### Repair a candidate on the currently bound source

A named proof can verify an existing predicate and write the proof candidate
for the current revision in the same invocation. Use different paths.

```sh
dive check edited/lockserv.tla --config lockserv.cfg --strategy symbolic \
  --property-proof Mutex --type-invariant TypeOK \
  --verify-predicate previous.json --write-predicate current.json
```

If the previous predicate passes, `closure_sweeps` is zero. If it is malformed
or fails induction, the checker searches for a new candidate on the already
bound model, with the time remaining in the original budget. Only a successful
inductive proof writes the destination. A reachable violation still goes
through independent replay. A successfully written authenticated candidate
records the obligations for the current revision.

For action-level diagnostics, keep both predicate files and the trusted report
from the combined call. Init and the named invariant are checked on each
revision. Each `closure_receipt.actions` entry identifies a compiled guard
and update. On a successful transfer with positive `reused_obligations`, the
intersection of the two authenticated lists identifies the reused images. The
remaining current entries were rechecked. A report with zero reuse means
every current image was checked. Changes in the number of images can follow
from expanding a quantified guard.


## Check a named inductive predicate

From a source checkout, pass the finite layout, candidate operator and one
configured safety property to the checker. It reads the bound TLA+ expressions
and expands the original `Next`. It proves initial inclusion, implication of
the property and preservation of every candidate conjunct on each constrained
step. It also checks the property on every generated destination that crosses
the constraint. An unwitnessed induction failure is reported as inconclusive.
The layout must represent every source variable and every possible destination
of an admitted source.

```sh
python3 scripts/symbolic/check_named_induction.py ProofTrial.tla ProofTrial.cfg \
  --exporter target/release/examples/symbolic_input --layout Layout \
  --candidate IndAuto --property Safety --save proof.json
```

Replace `--save` with `--verify` in a fresh process to bind the current program
and repeat all obligations. The compact certificate identifies the source
and lists every action with its enabled occurrences and constraint crossings.
The command uses the symbolic checker's Python dependencies.

On I3DP's three-server MongoLoglessDynamicRaft, the seven supplied strengthening
conjuncts covered **12,240,225 candidate valuations**. The checker proved all
four actions across **70,410,570 enabled occurrences** in **57.56 s** with
606 MiB peak RSS. A fresh process re-exported a relocated source checkout
and checked the certificate in 57.34 s. Independent replay accepted an eight-state path with
two elections, a membership change and propagation to other servers. Removing
one voting guard allowed two primaries in term 1 after two steps.
[Source, reproduction, controls and timings](../../conformance/reports/perf/2026-11-22-mongo-reconfiguration-induction.json)

### Retain authenticated source obligations across edits

A trusted local source worker can retain results for the exact obligations it
previously discharged. Set `DIVE_SYMBOLIC_PROOF_KEY` to the path of a private
32-byte file owned by the running user, with mode `0600` or `0400`. Keep that
key local to the processes allowed to establish proofs.

```sh
umask 077
python3 -c 'import secrets, sys; sys.stdout.buffer.write(secrets.token_bytes(32))' > proof.key
export DIVE_SYMBOLIC_PROOF_KEY="$PWD/proof.key"
python3 scripts/symbolic/source_obligations.py --backend bdd \
  --exporter target/release/examples/symbolic_input --layout Layout \
  --candidate IndAuto --property Safety ProofTrial.tla ProofTrial.cfg \
  --obligation-cache obligations.json
```

Run the same command in a new process after editing the original protocol
module imported by `ProofTrial.tla`. For example, disable the original
`Reconfig` branch of `Next` with a false conjunct and keep its other
branches and the CFG as published. Use the same receipt path, then restore
the module and repeat to check undo. Omit the cache argument for a cold
check. The [reproduction procedure](../../conformance/reports/perf/2026-11-22-mongo-reconfiguration-induction.json)
supplies the original Mongo trial and pins its source.

The BDD worker independently exports the current module closure and layout,
then checks Init, the safety implication and the constraint boundary. Each
retained conclusion binds the complete admitted premise and constrained
transition test along with the finite representation. An assumption or
strengthening change can invalidate an entire family of obligations. A
changed action can leave the obligations of other actions reusable.

For finite AsyncRaft, use the published log-invariant model and configuration
from the first section of this guide, with `--backend z3` and the same exporter
and optional cache argument. Its source worker recognizes the pinned protocol,
a comment-only revision and the documented edit disabling
`ClientRequestAction`. It rebinds the source and proves ground Init on each
request. An action group can be reused when its complete finite obligation
inputs and the proof graph agree. Other revisions require explicit source
admission. The wrapper reports the named proof, retained work, or an
inconclusive result. A missing or invalid local receipt is a cold run.

#### Reproduce an action edit and undo

Fetch the pinned I3DP checkout named in the [Mongo proof record](../../conformance/reports/perf/2026-11-22-mongo-reconfiguration-induction.json).
Use Python 3.12 or newer with `dd==0.6.0`, and build the `symbolic_input`
example. Point `I3DP` to its MongoLoglessDynamicRaft benchmark directory
and use a new directory for `WORK`.

```sh
PYTHONPATH=scripts/perf/scale python3 - "$I3DP" "$WORK" <<'PY'
import json
from pathlib import Path
import sys
from certify_reconfiguration import MODEL, PINS, sha, write_input

source, work = map(Path, sys.argv[1:])
assert all(sha(source / name) == pin for name, pin in PINS.items())
work.mkdir()
settings = json.loads((source / (MODEL + ".config.json")).read_text())
write_input(source, settings, work, 3)
PY
```

Use `"$WORK/ProofTrial.tla"` and `"$WORK/ProofTrial.cfg"` in the BDD
command above. After its first run, edit the generated directory's
`MongoLoglessDynamicRaft.tla`. Change the beginning of the action line
from `ReconfigAction == \E` to `ReconfigAction == FALSE /\ \E`. Run again
with the same cache, then restore the action and rerun. The strengthened
premises for the other three actions remain identical under this edit.

For AsyncRaft, copy the pinned published `AsyncRaft.tla`, `AsyncRaft.cfg`
and proof graph into a scratch directory. Use the outer worker's `z3` backend
with `z3-solver==4.15.4.0`. Create the effective CFG from the published
one with `MaxTerm = 2`, `MaxLogLen = 2` and `MaxMsgCount = 3`, and keep the
three original server values. Use the original `Init`, `Next` and
`StateConstraint`, select invariant `H_PrimaryHasEntriesItCreated` and
set `CHECK_DEADLOCK FALSE`. Run the source worker with the same cache on
both revisions. In the edited revision, change only the first `TRUE`
in `ClientRequestAction` to `FALSE`.

On one native c7i.2xlarge host, the first cached AsyncRaft action edit
retained ten action groups and took 2.21 s. Rechecking the same edited
source cold took 62.51 s. The Mongo action edit retained 240 obligations
and took 19.07 s versus 60.64 s cold. The measured time recovered the seed
cost by the first edit in both sequences. Mongo's lower wall time came with
higher peak resident memory. A weakened Mongo strengthening or a changed
assumption required all 360 obligations again. Restoring the source
recovered the previously checked results.
[Native measurements and checked controls](../../conformance/reports/perf/2026-11-23-source-induction-reuse.json).
