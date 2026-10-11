# 0019: Symmetry reduction

- **Status:** proposed
- **Date:** 2026-10-09

## Context

`SYMMETRY` is refused today (ADR 0015 §6). That refusal keeps 11 small-tier Examples models `unsupported`:

| Model | TLC golden (distinct / generated / depth) | Symmetry set |
|---|---|---|
| `CheckpointCoordination/MCCheckpointCoordinationFailure` | invariant violation, 88,152 / 1,302,999 / 12, trace 12 | `Permutations(Node)` |
| `FlashProtocol/MCFlashWithMutex` | 2,024 / 7,552 / 21 | `Permutations(NODE) \union Permutations(DATA)` |
| `GermanProtocol/MCGermanCMPWithMutex` | 1,107 / 3,281 / 19 | `Permutations(NODE)` |
| `GermanProtocol/MCGermanControl` | 735 / 1,946 / 19 | `Permutations(NODE)` |
| `GermanProtocol/MCGermanData` | 852 / 2,493 / 19 | `Permutations(NODE) \union Permutations(DATA)` |
| `MultiPaxos-SMR/MultiPaxos_MC_small` | 343,796 / 736,012 / 28 | `ConditionalPerm(...)` over three sets |
| `Paxos/MCPaxos` | 25 / 82 / 9, warning 2300 | singleton sets: TLC does **not** reduce |
| `Paxos/MCVoting` | 77 / 451 / 11 | `Permutations(MCAcceptor) \cup Permutations(MCValue)` |
| `SimplifiedFastPaxos/FastPaxos` | 25,617 / 320,053 / 22 | `Permutations(Values) \union Permutations(Replicas)` |
| `SimplifiedFastPaxos/Paxos` | 1,207 / 13,290 / 22 | the same |
| `braf/BufferedRandomAccessFile` | 3,316 / 128,581 / 10 | `Permutations(Symbols)` |

None of the 11 checks liveness. In the whole Examples corpus only `KeyValueStore/MCKVsnap` combines `SYMMETRY` with a liveness property. D-005 (owner decision, 2026-10-08) makes dive refuse symmetry with liveness by default, with TLC's warn-and-proceed under `--symmetry-liveness=tlc`.

TLC paths are relative to `tlatools/org.lamport.tlatools/src/` at tlaplus `94d0c50`. The probes are in `crates/dive-liveness/tests/fixtures/tlc-probes/` (names `Sym*`; the README has the transcripts' results).

### TLC's semantics

**1. Building the permutations** (`Tool.getSymmetryPerms`, `Tool.java:3836-3920`; run once at start-up).
- The cfg names one definition (`ModelConfig.java:255-268`). It is looked up among the definitions after cfg overrides, and errors are raised in this order:
  - not defined: `TLC_CONFIG_SPECIFIED_NOT_DEFINED` ("symmetry function");
  - not an operator definition: "The symmetry function X must specify a set of permutations.";
  - its value is not an enumerated set: "The symmetry operator must specify a set of functions.";
  - an element is not a function record (`FcnRcdValue`): "The symmetry values must be function records." A tuple written as a permutation is rejected;
  - a domain or range element is not a model value, identity pairs included: "Symmetry function must have model values as domain and range." (`MVPerms.java:26-39`). So `Permutations({1, 2})` is an error.
- **The group.** `MVPerms.permutationSubgroup` (`MVPerms.java:18-69`) treats the listed functions as generators. It drops identities and duplicates, then closes the set under composition to a fixpoint. `Permutations(A) \cup Permutations(B)` therefore yields the direct product Sym(A) × Sym(B), mixed products included. Probe `SymTwoSets` (two sets of two) reports 9 distinct states, the number of orbits under the product (3 × 3).
- **The identity** is not stored. It is the starting candidate of canonicalisation.
- **No bijectivity check.** A non-injective map such as `[a |-> b, b |-> b]` is accepted silently, and its closure is not a group.
- **"Too small" disables reduction.** If the definition's body has at least as many argument nodes as the group has elements, every argument is reported with the warning `TLC_SYMMETRY_SET_TOO_SMALL` (2300), and **no reduction is done at all** (`Tool.java:3862-3871`). The test is syntactic: for `Permutations(S)` the arguments are `[S]`. Examples `MCPaxos` hits it (two singleton sets: 2 functions, 2 arguments) and its golden counts are unreduced. A second check warns, with the same code, about `Permutations(X)` arguments none of whose members any permutation moves; that one keeps the group (`:3876-3907`).
- `-Dtlc2.tool.impl.ModelConfig.nosymmetry=true` turns symmetry off entirely (`ModelConfig.java:745-748`).

**2. The canonical state** (`TLCStateMut.fingerPrint`, `TLCStateMut.java:171-272`).
- It is the lexicographically smallest of the state and all its images under the group. States are compared variable by variable in TLC's variable order (`varLoc`, which is `Ir::tlc_var_order`; ADR 0015 §5), with `Value.compareTo`.
  - Model values compare by intern token (`UniqueString.compareTo`), not by name and not by `ModelValue.index`.
  - Sets compare by size, then by elements in normalized order; functions by domain, then values; records by field names, then values (`SetEnumValue.java:90-115`, `FcnRcdValue.java:148-215`, `RecordValue.java:302-331`).
- There is an early exit: a permutation is abandoned as soon as one of its permuted variables compares greater (`continue NEXT_PERM`, `:231`). It does not change the result.
- The fingerprint is FP64 over the minimum. **With a `VIEW`, the view is evaluated on the minimum**, so symmetry comes first and `VIEW` second (`:259-269`).
- Permuting a value (`Value.permute`) maps model values and rebuilds sets and functions over permuted domains. Lazy sets are enumerated first. Values with no `permute` (for example `KSubsetValue`, operator values) raise an error.
- The minimum is **recomputed on every `fingerPrint()` call**. Cost is O(|G| × state size), where |G| is the group order minus one.

**3. What is explored and counted** (`Worker.addElement`, `Worker.java:417-487`; `ModelChecker.DoInitFunctor`).
- Generated states are unchanged by symmetry.
- The seen set holds canonical fingerprints. **Distinct** = canonical fingerprints = orbits reached.
- The state TLC **enqueues and expands is the concrete successor that first reached the orbit**, not the canonical state.
- Constraints, invariants (on first occurrence), implied actions (on every transition) and deadlock are evaluated on concrete states.
- So, for a spec that is not really symmetric under the declared group, the counts depend on which member of each orbit arrives first. Probes `SymAsym` (only `p1` may count to 2) and `SymAsymLate` (only `p3`) differ only in which process is special. They report 7 distinct / 13 generated and 5 / 8. For a symmetric spec (probe `SymCounts`: 10 orbits, 21 generated, against 27 / 55 without symmetry in `SymCountsNone`), the counts are the orbit counts whatever the order.

**4. Counterexamples.**
- TLC rebuilds them by matching canonical fingerprints: the first initial state, then the first successor (actions in order) whose canonical fingerprint matches (`TLCTrace.getTrace`, `Tool.java:3719-3801`). Every printed step is a real transition, but the printed states may be different members of the orbits from the ones explored.
- Probes `SymViolation` and `SymOrder` print a 3-state trace for a depth-3 violation. ADR 0015 §4 already decided that dive prints the states it explored and that the harness compares length and validity.

**5. Liveness** (`AbstractChecker.java:118-122`; `LiveCheck.java:106-128`, `:552-578`, `:673-757`).
- TLC prints warning `TLC_FEATURE_UNSUPPORTED_LIVENESS_SYMMETRY` (2279) whenever there are implied temporal properties and a `SYMMETRY`, and then proceeds. `[]P` and `[][A]_v` property parts are safety checks and do not trigger it.
- The behaviour graph's nodes are canonical fingerprints. A node's state predicates are evaluated on the concrete state explored for it. Edge action predicates and tableau consistency are evaluated on the concrete successor. Once a target is on disk, a second edge to the same canonical target is dropped with its predicate bits.
- **It misses real violations.** Probe `SymLiveMissed`: three model values, `x' \in S \ {x}` under `WF_x(Next)`, `PROPERTY []<>(x = a)`. The behaviour a, b, c, b, c, ... violates it. With `SYMMETRY`, TLC reports **success** (1 distinct state, warning 2279). Without it (`SymLiveMissedNone`), TLC reports the violation, a 3-state lasso back to state 2.
- **It also reports false violations.** Probe `SymLiveSymProp`: the token alternates between two processes under `WF_tok(Move)`; `\A p \in S : []<>(tok = p)` holds, and both the spec and the property are symmetric. With `SYMMETRY`, TLC reports `Temporal property Fair was violated` with a counterexample of one state followed by "Stuttering". No such behaviour exists: the spec's fairness forbids stuttering there. The quotient graph has one node whose self-loop is a real `Move` step, and TLC prints the self-loop as stuttering.
- Survey probe P4 (`SymLiveWarn`): `<>(x = b)` holds and TLC reports success, so the quotient is not wrong in every case. It is unpredictable, which is the point of D-005.
- In simulation mode, TLC prints no warning.

## Decision

### 1. Canonicalisation in dive

Symmetry is a canonicalisation step in `dive-check`'s dedup-key hook (ADR 0015 §6). The hook computes, for each in-model successor in phase 1:

1. `κ(t)`, the canonical state: the minimum of `t` and its images under the group;
2. the dedup key of `κ(t)`, or of `VIEW(κ(t))` when there is a `VIEW`.

The search keeps exploring the **concrete first occurrence**, as TLC does. Successors are generated from it, its constraints, invariants and deadlock are evaluated on it, and the counterexample prints it. Because 𝒪 is TLC's single-worker order under `--compat=tlc`, the explored member of each orbit is TLC's, so the counts of asymmetric specs (`SymAsym`, `SymAsymLate`) also match.

**The order used for "minimum".**
- **Under `--compat=tlc`:** TLC's exactly. Variables in `Ir::tlc_var_order`, values by `dive-value`'s TLC comparison, model values by TLC's intern token. Then `κ(t)` is TLC's minimum bit for bit, and so is the state fingerprint (`StateFingerprinter::tlc`). That matters in three places: the classes when a `VIEW` is not invariant under the group (the view of TLC's minimum decides the class); TLC's printed "actual" collision probability; and the randomness seeded from fingerprints (ADR 0014 §7).
- **In the default profile:** the run's value order (canonical, D-006) and the IR's variable order. The partition into orbits does not depend on which member is chosen as minimum, so distinct and generated counts are the same as under `--compat=tlc`. There are two exceptions, both flagged in the output as "may differ under `--compat=tlc`" (ADR 0010 §3):
  - a `VIEW` that is not invariant under the group;
  - a spec that is not symmetric under the declared group, explored in canonical order (D-010), whose explored representatives can differ from TLC's (as in `SymAsym`). dive cannot detect asymmetry cheaply in general, so the flag is raised for every complete search with `SYMMETRY` whose enumerated sets had elements whose two orders disagree. This is ADR 0015 §7's existing D-010 detector, extended from "stopped at a violation" to "complete search with `SYMMETRY`".

**The group.** dive ports `MVPerms.permutationSubgroup`: generators from the evaluated set, identities and duplicates dropped, closure under composition to a fixpoint, and TLC's too-small heuristic, including disabling reduction. Errors and warnings use TLC's codes and texts. Permutations act on model-value tokens (`dive-value`'s `ModelValue` id), held as dense arrays like `MVPerm`.

**Non-bijective "permutations".** TLC accepts them and then reduces unsoundly. dive builds TLC's behaviour under `--compat=tlc`. In the default profile dive would rather refuse them with a diagnostic naming the offending function. That is result-changing, so it is proposed to the owner (open question, below) and not adopted. Until it is answered, both profiles match TLC and dive prints a warning that TLC does not.

### 2. Cost

The work per successor is TLC's O(|G| × state), and dive does it once per in-model successor, in parallel phase 1. TLC repeats it on every `fingerPrint()` call.

- **Early exit and lazy permutation.** Variables are permuted one at a time in comparison order, and a candidate is abandoned at the first variable that compares greater, as in TLC. A permuted value is built only when the comparison needs it.
- **Model-value occurrence filter.** A value containing none of the model values the group moves is its own image; this is precomputed per value through `dive-value`'s cached hash-consing metadata, where available.
- **Cached canonical keys.** A state's key is computed once and stored with the candidate. Reconstruction (ADR 0015 §4) replays by successor ordinal and never needs fingerprints.
- **Faster canonical forms, later.** For the common case of one fully symmetric set whose model values appear only as function domains, a sort-based canonical form is O(n log n) rather than O(n!). It is result-preserving in the default profile for complete searches, because only the partition matters. It is not usable under `--compat=tlc`, where the fingerprint must be TLC's minimum, nor with a `VIEW` not invariant under the group. It is a performance work package, after the exact path passes.

### 3. Counterexamples and validation

Counterexamples print the explored concrete states, a valid behaviour, and `dive-trace` validates them like any other (ADR 0016). The length equals TLC's because both are breadth-first paths in the same 𝒪. Content can differ from TLC's rebuilt trace, which the harness does not compare.

Symmetry plus liveness is covered in ADR 0018 §9: a lasso whose loop closes on an orbit, not on a state, carries the permutation that closes it, and the validator checks that.

### 4. Symmetry with liveness (D-005)

- **Default, `--symmetry-liveness=refuse`:** dive refuses the run before checking, with error `DIVE-L0010`. The error names the `SYMMETRY` definition and the temporal properties, explains that TLC can both miss violations and report false ones in this combination (pointing to `SymLiveMissed` and `SymLiveSymProp` in `docs/tlc-differences.md`), and suggests checking liveness in a separate run without `SYMMETRY`. Safety-only `PROPERTY` parts (`[]P`, `[][A]_v`) do not trigger it, matching TLC's warning condition.
- **`--symmetry-liveness=tlc`** (part of `--compat=tlc`): warning 2279, then TLC's quotient-graph construction exactly as ADR 0018 §9 specifies. The probes `SymLiveWarn`, `SymLiveMissed` and `SymLiveSymProp` are its conformance tests, and must reproduce TLC's verdicts, including the missed violation and the false one.
- **A sound alternative, proposed and not adopted:** `--symmetry-liveness=unreduced`, which ignores `SYMMETRY` for a run that checks liveness. The counts are then those of the unreduced spec, and the output says so. It answers the question the user meant to ask. It is result-changing relative to TLC (different counts, and possibly a different verdict), so it is an open question for the owner. It would also be a candidate default instead of refusing.

### 5. Simulation

Symmetry does not change simulation's safety checking, since simulation does no deduplication. It affects liveness on simulated traces. Simulation is a later milestone, and this ADR's canonicalisation is reused there.

### 6. Diagnostics dive adds

- Non-bijective symmetry functions: a warning in both profiles (§1).
- `--check-symmetry` (opt-in, result-preserving, reports only): for every explored state `s` and every generator `π`, check that the successors of `π(s)` are the images of the successors of `s`, and that the invariants, constraints and `VIEW` are invariant under `π`. Any failure is reported as "the declared symmetry is not a symmetry of the spec", naming the state and the generator. TLC trusts the declaration silently, and an unsound declaration silently changes counts (`SymAsym`) and can hide violations. It at least doubles the cost, so it is opt-in. Whether it should become a default for small models is left open.

## Consequences

- The 11 small-tier symmetry models become checkable. Under `--compat=tlc` each must match its golden on every field; `MCCheckpointCoordinationFailure`'s 12-state counterexample must validate.
- Under `--compat=tlc`, symmetry needs TLC-exact value comparison, TLC's intern order for model values, and `Ir::tlc_var_order`. All three exist from M5 (ADR 0012, ADR 0015 §5).
- **Every probe is a test**, at 1, 4 and 8 workers and under chaos, in both stores: `SymCounts`, `SymCountsNone`, `SymAsym`, `SymAsymLate`, `SymOrder`, `SymTwoSets`, `SymView`, `SymViolation`. Add a too-small probe and an error probe for each TLC symmetry error when implementing.
- `docs/tlc-differences.md` D-005 gains the measured evidence (`SymLiveMissed`, `SymLiveSymProp`).
- **Open questions for the owner** (`docs/open-questions.md`):
  - Refuse non-bijective symmetry functions in the default profile?
  - Add `--symmetry-liveness=unreduced`, and should it replace `refuse` as the default?
- **Revisit:**
  - when simulation is built;
  - after the first symmetry benchmarks (the sort-based canonical form).

## Owner decisions, 2026-10-09

The intended default is `--symmetry-liveness=unreduced`, disabling reduction for the entire liveness run and reporting unreduced counts. This supersedes §4's refusal default; `refuse` stays an option and `--compat=tlc` retains the quotient path. Reject non-bijective functions in the default profile; preserve TLC's acceptance in compat mode. Build `--check-symmetry` as an opt-in audit. These are approved designs, not claims that the currently unsupported features are implemented.

## S1 closure increment, 2026-10-09

The public `dive_model::symmetry` kernel closes ordered model-value index maps
with TLC's exact composition/insertion order. Identity is implicit; duplicate
generators are removed. Checked mode rejects non-bijections after identity
extension; TLC mode retains their finite closure. Universe, map, dense-cell
and work limits refuse without exposing partial groups. A direct pinned Java
driver supplies 1,329 order-sensitive controls, including malformed maps.

The Java kernel throws for fewer than two supplied functions; the Rust kernel
returns `TooFewGenerators`. A raw normal-CLI SyEmpty capture records GENERAL,
exit 255. Tool-level lookup, normalization and too-small/unused-set warnings
remain open, as do canonicalization, reduction and symmetric trace evidence.
No SYMMETRY checker refusal is lifted by this increment.

Nine additional raw binding/error/warning captures include SyNonBiject:
TLC reports success with a collapsing map (3 generated / 1 distinct / depth 1),
while the unreduced control reports the real Inv violation (3 / 2 / depth 2).
They establish the next binding stage's evidence, not its implementation.
Issue #104 tracks the remaining binding, warnings and canonicalization work.

## S1 declaration increment, 2026-10-10

`Model::symmetry_declaration` prepares pure constant declarations without
changing normal checking. The configured name's lookup is retained immediately
after `processSpec`, before `processConstantDefns` replaces definitions with
values, matching `SpecProcessor`'s snapshot and `Tool.getSymmetryPerms`.
Configuration substitutions and assignments remain distinguishable. The
original definition body is evaluated, and only normalized enum sets of function
records are accepted. Tuple and record literals retain their separate TLC
representation errors.

Four early errors match the raw pinned transcripts (2229 for the missing name;
GENERAL for a non-definition, non-enum-set or non-function-record). Empty,
singleton, non-model-value and malformed-map declarations pass this **intermediate**
representation stage. Too-small warnings must precede domain/closure validation,
so they are deliberately not preempted here. Effectful, parameterized and
nonconstant definitions refuse by name; evaluator bugs stay internal errors.
The API is called explicitly and does not activate symmetry reduction. Warning
construction, model-value validation, group attachment, canonical state keys and
concrete trace reconstruction remain open in #104.

Unverified declaration-evaluation and normalization startup errors refuse by
name rather than reuse an evaluator diagnostic as an exact startup message.
A division-by-zero control verifies that no TLC code is claimed for that refusal.

The raw SyDivZero and SyMixedElements captures prove outer GENERAL, exit 255,
including the former's nested 2179 message. API controls require explicit
refusals for both; startup wrapping must not be inferred from evaluator codes.

## S1/S2 pure safety increment, 2026-10-10

`Model::prepare_symmetry` now implements source-backed argument warnings, the
TLC too-small-before-domain-check ordering, model-value domain/range validation,
bounded closure, and unused direct `Permutations` argument warnings. It attaches
no group when TLC disables reduction. Checked mode validates model-value maps even
when reduction would be disabled; non-bijections refuse before search. Argument
syntax not mapped to SANY's operator arguments and unverified startup exceptions
remain named refusals. The source token image strips only lexer trivia, preserving
strings and original spelling; reverse configuration substitutions retain the
specification's warning alias. `FileInfo` now retains module text, and IR schema
5 invalidates old serialized caches; the crate dependency graph is unchanged.

Pure safety checking prepares this stage before assumptions, then removes only
the satisfied symmetry refusal. The shared key hook canonicalizes in TLC variable
and value order in compat mode, or IR variable and canonical value order by
default. `VIEW` sees the canonical state. The queue, constraints, invariants,
predecessor ordinals and printed traces retain the original concrete states.
TLC normalization side effects are preserved after canonical fingerprinting and
before viewing, including normalization of the concrete state. Default complete
symmetry searches report the D-010 order policy even when no violation stops them.

The current value permutation covers scalars, model values, enumerable sets,
tuples, record literals and normalized functions, including nested model values
in domains. Deferred/lazy value permutation, random seeding under symmetry,
symmetry-liveness reduction and the symmetry audit remain explicit gaps. Limits
refuse without using a partial group or partially canonicalized key. Each state
has bounded permutation/normalization work and a depth ceiling of 256.

Eight clean TLC safety probes match verdicts, counts, depth and trace length in
both stores at 1/4/8 workers with chaos. Actual SymOrder/SymViolation checker
traces validate independently in both profiles and stores at every width;
corruption controls reject. Three new normal pinned captures verify conditional
argument plurality, reverse configuration aliases, and a singleton non-model
permutation argument whose too-small warning suppresses domain validation in
TLC mode. SyNonBiject matches TLC's unsound success under compat and refuses by
name before search under the default profile. These are staged S1/S2 results;
remaining startup contexts are not declared complete.

`SyInfinite` is a negative control for the unsupported boundary: a state
containing `Nat` makes pinned TLC report 2102, exit 75, before storing its
initial key. Infinite `UserValue` sets inherit the rejecting `Value.permute`.
Both dive stores and profiles refuse these unverified permutation contexts;
they are not silently treated as identity permutations. The raw transcript
and a checker regression cover this refusal at 1/4/8 workers.

The staged corpus study is `conformance/reports/studies/symmetry-safety.md`.
All eleven manifest-backed safety symmetry targets agree at four workers;
the filters additionally select a distinct Voting model (12/12 total).
The original single-worker FastPaxos timeout is retained in the initial raw
report. Shared machine load and differing captured binaries make these
functional conformance results, not performance benchmarks. These results do
not close the deferred/liveness/random/startup contracts above.

## Implementation addendum: pure VIEW normalization (2026-10-11, #188)

`TLCStateMut.fingerPrint` normalizes every concrete variable before VIEW even
without a symmetry declaration. The shared key path now follows that order;
normalization applies to ignored variables too. Concrete predecessor retention
and VIEW-after-symmetry order are unchanged. The bounded helper already used
by safety symmetry is reused without a crate dependency change. Unproved
normalization/error-class cases refuse explicitly instead of guessing EC1000
or reporting a model verdict. Paired primary/native evidence, compound
controls, actual native recovery and independent concrete witness controls
are retained in `conformance/probes/view-concrete-normalization/`. No random
VIEW, lazy-value, full memory bound or checkpoint certificate claim is added.
