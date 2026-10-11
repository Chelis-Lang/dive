# TLC parity ledger

Inventory baseline: main `217cebfa8bd35735bf14d38f28a4ae17628739aa`, 2026-10-10.
Implementation rows also reflect subsequent recorded increments.

The [delivery plan](design/tlc-parity-plan.md) covers the entire pinned TLC
capability set through a native Rust checker, including simulation, DFID,
distribution, recovery and PlusCal. This ledger records the denominator and
evidence without converting partial implementations into completed features.

## What the denominator means

The maintained [semantic ledger](../conformance/parity-features.json) has **37
stable feature families**: 7 have an implemented core, 26 are partial, and 4 are missing. Those implementation labels do not certify full parity.
A broad family may contain both passing cases and unresolved interactions.
**No family-level certification record has yet been completed.** Existing
model and differential evidence remains valid; this initial audit must not be
reported as either zero implementation progress or a completion percentage.

Report certified families out of 37 only after their complete acceptance
matrix passes. The separate literal surface counts below overlap: flags can
activate the same family, a configuration keyword can select a mode, and a
module can contain many operators. Never add those counts, weight them into
an invented percentage or equate a registry row with implemented semantics.

| Source surface | Count | Extraction authority |
|---|---:|---|
| TLC CLI spellings, including aliases and internal controls | 46 | `tlc2/TLC.java` parameter branches |
| SANY CLI spellings, including case aliases | 14 | `tla2sany/drivers/SANY.java` |
| PlusCal translator CLI spellings | 21 | `pcal/trans.java`, executable option branches |
| Configuration keyword spellings | 22 | `ModelConfig.ALL_KEYWORDS` |
| SANY builtin operator records, including syntax/proof nodes and aliases | 72 | `BuiltInOperators.Properties` |
| Native override registration records | 158 | Bytecode-derived `dive-ir` registry |
| Standard module source files | 20 | Pinned `StandardModules/*.tla` |
| CommunityModules source files | 34 | Pinned `modules/*.tla` |

The [surface inventory](../conformance/parity-surfaces.json) retains every
name, flag source lines, operator arity/level, override calling/installation
kind, Java method provenance, source-file hashes and module-file hashes.
Override registry labels are **119 Core, 12 Extension, 8 Introspection and
19 Io**. `Core` means intended scope, not successful implementation.
All 158 bindings remain `audit-required` until their cases are mapped.

This is an initial source-surface inventory, not a claim that every Java
system property, nested option argument, runtime API or module definition has
already received an individual case. Complete those subcase mappings before
certifying the responsible family. In particular `-simulate` schedulers,
`-dump` variants, `-lncheck` values, message lists and library/override
installation need more cases than one flag row. Native custom overrides
replace arbitrary JVM plugin loading; Java worker/checkpoint interoperability
is explicitly outside this milestone. Reals/proof constructs that TLC cannot
execute require matching rejection, not fabricated execution.

Simulation evidence now covers ordinary pure finite safety sampling: ten seeded/
aril selection controls and twelve error probes. This does not certify the
whole F-024 family: temporal simulation, runtime effects, generation, other
schedulers and parallel/distributed simulation remain named refusals.

## Semantic feature families

Each JSON row owns source/Rust references, acceptance criteria, interactions,
existing issues and its phase in the delivery plan. `implemented` describes
the core path; it does not remove the listed certification obligations.

`python3 -I scripts/check-parity-ledger.py` checks these references, stable IDs,
interactions and the Markdown census offline in the Workspace gate. Its mutation
controls reject misleading accounting. Passing this structural check does not
certify semantics; family certification needs its separately reviewed case matrix
and a corresponding certification schema before the label can change.

| ID | Family | Implementation | Phase | Current evidence |
|---|---|---|---:|---|
| F-001 | TLA+ grammar and lossless parsing | implemented | 1 | [frontend-campaign.md](frontend-campaign.md) |
| F-002 | Semantic resolution, levels and module/proof syntax | implemented | 1 | [frontend-campaign.md](frontend-campaign.md) |
| F-003 | Configuration parsing | implemented | 1 | [tlc_oracle.rs](../crates/dive-config/tests/tlc_oracle.rs) |
| F-004 | Model/configuration binding and startup | partial | 6 | [startup.rs](../crates/dive-model/tests/startup.rs) |
| F-005 | Values, equality, order, laziness and representation | partial | 6 | [tlc_differential.rs](../crates/dive-value/tests/tlc_differential.rs) |
| F-006 | Evaluation scopes, recursion and higher-order operators | partial | 3 | [temporal.rs](../crates/dive-api/tests/temporal.rs), [recursive refusal probes](../conformance/probes/finite-deferred-audit/README.md) |
| F-007 | Finite domains, successor enumeration and independent ENABLED | partial | 3 | [constructor_covers.rs](../crates/dive-api/tests/constructor_covers.rs), [deferred_domains.rs](../crates/dive-api/tests/deferred_domains.rs) |
| F-008 | Native override discovery and installation | partial | 6 | [lower.rs](../crates/dive-ir/tests/lower.rs) |
| F-009 | Standard-module operators | partial | 6 | [tlc_fixtures.rs](../crates/dive-eval/tests/tlc_fixtures.rs) |
| F-010 | CommunityModules operators and TLA+ fallback definitions | partial | 6 | [README.md](../crates/dive-eval/tests/fixtures/examples/README.md) |
| F-011 | Error classes, construction provenance and precedence | partial | 6 | [errors.rs](../crates/dive-check/tests/errors.rs) |
| F-012 | Sequential BFS safety, invariants and deadlock | implemented | 2 | [probes.rs](../crates/dive-check/tests/probes.rs) |
| F-013 | Deterministic parallel checking and runtime-sensitive fallbacks | implemented | 4 | [invariance.rs](../crates/dive-check/tests/invariance.rs) |
| F-014 | Fingerprint/exact stores and collision accounting | implemented | 7 | [tlc_state_fingerprints.rs](../crates/dive-store/tests/tlc_state_fingerprints.rs) |
| F-015 | State/action constraints and action-property split | implemented | 4 | [probes.rs](../crates/dive-check/tests/probes.rs) |
| F-016 | VIEW identity and temporal lifting | partial | 4 | [m6-plan.md](design/m6-plan.md) |
| F-017 | Symmetry declaration binding, warnings and closure | partial | 2 | [symmetry.rs](../crates/dive-check/tests/symmetry.rs): supported pure warnings; startup gaps remain |
| F-018 | Symmetry reduction, concrete traces and audit | partial | 2 | [12-model safety study](../conformance/reports/studies/symmetry-safety.md), [trace controls](../crates/dive-api/tests/symmetry_trace.rs); liveness and audit remain refused |
| F-019 | Temporal language, simplification and exact tableaux | partial | 3 | [planning.rs](../crates/dive-liveness/tests/planning.rs) |
| F-020 | Temporal graph capture and phase-L timing | partial | 4 | [temporal-study.md](temporal-study.md) |
| F-021 | WF/SF fairness and independent periodic validation | partial | 4 | [temporal.rs](../crates/dive-api/tests/temporal.rs) |
| F-022 | SCC acceptance, TLC lasso selection and attribution | partial | 4 | [lassos.rs](../crates/dive-liveness/tests/lassos.rs) |
| F-023 | Periodic/final/off/sequential liveness schedules | missing | 4 | [m6-plan.md](design/m6-plan.md) |
| F-024 | Simulation, generation, scheduling and random replay | partial | 5 | [seeded controls and independent validation](../crates/dive-api/tests/simulate.rs), [bounded scope](../crates/dive-sim/README.md); #111 |
| F-025 | DFID checking | partial | 5 | [DFID controls](../conformance/reports/dfid-primary-controls.json); pure safety only, uncertified |
| F-026 | Granted environment, file and process effects | missing | 6 | [tlc-differences.md](tlc-differences.md) |
| F-027 | Runtime introspection and configuration services | partial | 6 | [tlc_module.rs](../crates/dive-check/tests/tlc_module.rs) |
| F-028 | CLI/output conventions and message controls | partial | 6 | [sprint-status.md](sprint-status.md) |
| F-029 | Trace/state dumps, ALIAS and trace exploration | partial | 6 | [trace_controls.rs](../crates/dive-api/tests/trace_controls.rs) |
| F-030 | Coverage and debugger services | missing | 6 | [tlc-parity-plan.md](design/tlc-parity-plan.md) |
| F-031 | Bounded-memory stores, frontiers, graphs and SCC spill | partial | 7 | [frontier controls](../crates/dive-check/tests/frontier_spill.rs), [predecessor controls](../crates/dive-api/tests/predecessor_spill.rs); [frontier limits](frontier-storage.md), [predecessor limits](predecessor-storage.md); seen/candidate/graph/SCC retention remains resident |
| F-032 | Native checkpoint and recovery | partial | 7 | [native-checkpoint.md](native-checkpoint.md) |
| F-033 | Native distributed checking and recovery | missing | 7 | [tlc-parity-plan.md](design/tlc-parity-plan.md) |
| F-034 | PlusCal translation and translation validation | partial | 8 | [pluscal-normalization.md](pluscal-normalization.md): bounded sequential artifacts; full translation pending; [bounded Full checksum editing](pluscal-rewriting.md), broader AST checksum scopes/file format pending |
| F-035 | Public Rust APIs and cargo-test integration | partial | 9 | [temporal.rs](../crates/dive-api/tests/temporal.rs), [simulation API](../crates/dive-api/tests/simulate.rs); #111; [bounded Full checksum editing](pluscal-rewriting.md), broader AST checksum scopes/file format pending |
| F-036 | Complete corpus, harness and release certification | partial | 1 | [conformance-corpora.md](conformance-corpora.md) |
| F-037 | Global TLC profile and approved better defaults | partial | 9 | [tlc-differences.md](tlc-differences.md) |

## Evidence that may be claimed now

- **Strict models:** [the committed standard report](../conformance/reports/20261009T134501007-dive-small.json)
  records 83 agree and 71 explicit refusals over 154 Examples/probe entries,
  with no disagreements, timeouts or pending validation. This is a fixed
  selected corpus, not 83 complete TLC features.
- **Temporal study:** [the 53-model study](temporal-study.md) records 26
  truth/count matches, 22 refusals and five timeouts using explicit
  `--lncheck final --lasso short`, labeled `tlc+overrides`. These matches
  do not certify phase-L stopping, exact temporal classes, property selection,
  exact lassos or parallel graph capture for that historical run. The current
  [bounded parallel capture increment](temporal-api.md#bounded-parallel-graph-retention)
  adds inline/1/4/8/chaos controls and truthful pool widths; F-020 remains partial.
- **Frontend:** [the deep campaign](frontend-campaign.md) agrees on 1,108
  selected inputs in both direct grammar and full SANY checking, retaining
  the difference between those measurements. It is a bounded campaign.
- **ValidationTestSuite:** [the smoke import](conformance-corpora.md) accounts
  for all 36 selected cases and provides 16 generated models at 1/4/8
  workers. The entire source suite and TLC JUnit model corpus remain open
  work under #2.
- **Symmetry:** declaration/closure tests retain 1,329 direct Java controls.
  [The staged safety study](../conformance/reports/studies/symmetry-safety.md)
  fully agrees on all eleven intended symmetry models plus the additionally
  selected Voting model at four workers. The original single-worker FastPaxos
  timeout remains unchanged. Both profiles/stores have pure warning, key and
  concrete trace controls at 1/4/8 workers; the 21-model required CM1 gate now
  includes symmetry-bearing Paxos Voting. The preserved default-profile study
  agrees on two selected models. Unverified startup exceptions, unsupported
  declaration shapes, deferred/lazy/infinite permutations, randomness under
  symmetry and symmetry with liveness remain named refusals. The optional
  symmetry audit is unimplemented. #104 and both families remain partial.

Failures remain in their original raw reports. Their unreliable zero-worker
fallback must not be used to infer that no search occurred. The #109 correction
retains execution stage, completed capture facts and actual workers on new
temporal failures; lost thread evidence is unknown, never a proved zero.
It does not improve the historical study totals. See
[temporal_evidence.rs](../crates/dive-api/tests/temporal_evidence.rs).
Busy-host timings do not certify a throughput or scaling target.

## Literal flags and backend ownership

The exact spelling is inventoried even when dive will use a clearer native
option or subcommand. A mapped capability must retain TLC-compatible
semantics, validation and diagnostics. Flags currently understood by the
harness when it launches Java are not thereby implemented by the Rust CLI.

| Pinned TLC spellings | Families / implementation obligations |
|---|---|---|
| `-modelcheck`, `-workers`, `-deadlock`, `-config` | F-003/F-012/F-013; core implemented, remaining option/error interactions require audit |
| `-seed`, `-aril`, `-fp` | F-014/F-024; Java random kernel and fingerprint pin exist, all modes/aril CLI coverage still required |
| `-lncheck` | F-019–F-023; only explicit final/short mixed path currently checks temporal models |
| `-simulate`, `-generate`, `-depth`, `-dfid` | F-024/F-025; pure finite single-worker simulation and bounded pure-safety DFID are partial; generation and the full scheduler/mode matrix remain pending |
| `-checkpoint`, `-recover`, `-metadir`, `-cleanup` | F-031/F-032; bounded native completed-level safety recovery; broader persistence interactions pending |
| `-maxSetSize`, `-fpmem`, `-fpbits`, `-gzip` | F-005/F-014/F-031; TLC limit and storage policy behavior must be mapped and tested |
| `-continue`, `-postCondition` | F-012/F-027; continuation, terminal effects and their error order need implementation/audit |
| `-dump`, `-dumpTrace`, `-loadTrace`, `-view` | F-016/F-029; structured validated traces exist, full TLC formats/loading/view interactions pending |
| `-generateSpecTE`, `-noGenerateSpecTE`, `-noTE`, `-noGenerateSpecTEBin`, `-noTEBin`, `-teSpecOutDir` | F-029; trace-exploration assets and native binary format pending |
| `-coverage`, `-debugger`, `-debug`, `-inv`, `-invlevel` | F-030; source coverage and debugger/internal diagnostic controls pending |
| `-difftrace`, `-terse`, `-tool`, `-nowarning`, `-suppressMessages`, `-messagesAsErrors`, `-userFile` | F-011/F-028; all message/printing controls and their observable effects pending |
| `-help`, `-h` | F-028; dive help exists, full argument/rejection mapping remains part of CLI audit |

SANY surfaces include `-S/-s`, `-L/-l`, `-G/-g`, `-D/-d`,
`-STAT/-stat`, `-error-codes`, `-suppressmessages`, `-messagesaserrors`
and `-help`. Accepted casing, parse/semantic/level toggles and output behavior
belong to F-001/F-002/F-028; `dive analyze` acceptance alone does not prove
every driver control.

PlusCal surfaces are `-help`, `-writeAST`, `-spec`, `-myspec`, `-spec2`,
`-myspec2`, `-debug`, `-unixEOL`, `-termination`, `-noold`, `-nocfg`,
`-noDoneDisjunct`, `-wf`, `-sf`, `-wfNext`, `-nof`, `-label`,
`-reportLabels`, `-labelRoot`, `-version` and `-lineWidth`.
All belong to F-034, which now has a bounded native C/P source AST; macro/label
normalization, translator options and translated output remain pending. Commented-out
`-readOnly` and `-writable` branches are not counted as accepted options.

## Configuration parsing versus execution

| Keyword spellings | Backend obligation |
|---|---|
| `CONSTANT`, `CONSTANTS` | F-004: literal/model/operator replacements and module-local substitution |
| `INIT`, `NEXT`, `SPECIFICATION` | F-004/F-012/F-019: safety/temporal roles, startup and order |
| `CONSTRAINT`, `CONSTRAINTS`, `ACTION_CONSTRAINT`, `ACTION_CONSTRAINTS` | F-015/F-020: filtering/check timing and graph retention |
| `INVARIANT`, `INVARIANTS`, `PROPERTY`, `PROPERTIES` | F-004/F-012/F-019/F-021: state/action/temporal checking and independent attribution |
| `VIEW` | F-016: safety identity and temporal closing evidence |
| `SYMMETRY` | F-017/F-018: warnings, canonicalization, trace and liveness policies |
| `ALIAS` | F-029: evaluation and presentation without silent run perturbation |
| `POSTCONDITION`, `POSTCONDITIONS` | F-027: terminal evaluation, effects and errors |
| `_PERIODIC`, `_RL_REWARD`, `_POSSIBLE` | F-024/F-027: runtime hooks and their mode-specific semantics |
| `CHECK_DEADLOCK` | F-012/F-025: mode/config/CLI precedence and independent successor evidence |

All 22 spellings are in the parser inventory. Backend certification must
exercise each directive, aliases, invalid values and interacting directives.

## Standard and CommunityModules audit

The JSON records the exact module files and every override binding. Audit
TLA+ fallback definitions as well as installed native overrides. Module-level
ASSUMEs are useful controls, but do not cover every operator argument/error
case, lazy normalization path, cached representation or runtime phase.

| Module group | Current evidence and remaining obligation |
|---|---|
| Naturals, Integers, Strings, Sequences, FiniteSets | Core override fixtures exist; complete valid/invalid classes, construction provenance and mixed-module override precedence remain certification work |
| TLC, Randomization, TLCExt | Print/Get/Set and bit-exact random paths exist; aril/mode coverage, introspection, caches/deferred calls and terminal/temporal effects remain open |
| Bags/BagsExt, Bitwise, FiniteSetsExt, Functions, Graphs/UndirectedGraphs, SequencesExt, DyadicRationals, IOUtils atoi | Native evaluator paths exist with named unsupported cases; order-sensitive folds and error variants need operator-by-operator audit |
| Json, CSV, IOUtils host operators and binary trace services | Pure JSON conversion exists; file/environment/process and trace serialization require native grants and differential controls |
| Combinatorics, GraphViz, SVG, Statistics, VectorClocks and TransitiveClosure overrides | Registered extension bindings; registry scope is not evidence of implementation |
| Reals, RealTime, Toolbox and internal trace/possible modules; remaining CommunityModules TLA+ definitions | Determine executable TLC cases, fallback evaluation and oracle rejection individually; account for every inventoried file |

## Keep the ledger trustworthy

1. Maintain stable F-IDs; never drop a difficult family or reuse an ID to
   improve the denominator. Split a family only with a reviewed migration
   that preserves every obligation and explains the new counts.
2. Update implementation status, acceptance, issue links and evidence in
   `conformance/parity-features.json` alongside code. Keep this summary in sync.
   Record affected source surfaces and interactions for every increment.
3. Certification requires a committed matrix covering mapped source surfaces,
   positive/negative controls, both profiles/stores, applicable worker/mode
   combinations and an independent review. Record binary/pin/schema/selection
   identities and the validating report; `audit-required` is not certified.
4. Preserve timeouts, unsupported features and pending validation in totals.
   No selective rerun may erase an original finding. Default-profile differences
   need the registry and matching maintained D-entry in the same PR.
5. Only oracle-generated files under `conformance/golden/` are goldens. These
   inventories are planning metadata, not hand-authored oracle results.

Reproduce/check the literal inventory using clean source checkouts at the
commits in `conformance/oracles.toml` and `conformance/corpora.toml`:

```sh
python3 -I scripts/parity-inventory.py \
  --tlc-source "$DIVE_TLC_SOURCE" \
  --community-source "$DIVE_COMMUNITY_SOURCE" --check
```

The two variables identify checkout roots, not their `src`/`modules` children.
Without `--check`, the script refreshes only `conformance/parity-surfaces.json`.
It strips Java comments while preserving strings and line offsets, checks
clean checkout revisions and expected census counts, and writes no timestamps.
Source drift or unrecognized configuration syntax fails rather than silently
shrinking the inventory. Review changes and their acceptance mapping before
moving pins or changing expected counts.

The harness resolves relative implementation executable paths before changing
child model directories (F-036), with a regression control covering that launch
path. This fixes execution plumbing; it does not certify a checking family.

F-007 now includes budgeted finite lazy constructor controls. Its complete
[ten-model follow-up](lazy-domain-study.md) records zero completed corpus matches,
seven named refusals, two resource bounds and one timeout. These controls widen
the implementation without completing or certifying the domain family.


F-007 also includes six paired captured-filter/map/state-dependent-domain probes
checked in 72 final/short profile/store/requested-worker settings, with fresh
independent property-word controls and fully validated violating reports. Input
work is charged before predicates run; original capture scopes, profile order
and priming are retained. Exact phase-L semantics and full domain coverage are
not certified. The ten-row study above is unchanged until a new immutable
whole-selection report accounts for every outcome.

The same probe group retains two recursive LET function controls that remain
explicitly refused in both profiles, with the exact opaque scoped-level diagnosis.
This is F-006 gap evidence, not recursive or higher-order implementation credit.
Both family statuses remain partial and require audit before certification.


F-007 captured-filter purity also refuses opaque Rust operator captures before
predicate evaluation: call-site IR effects cannot prove a programmatically
supplied operator pure. The retained native repro and both-profile no-output
regression document this F-006 limit without changing family statuses.

Missing captured-expression metadata supplied through the Rust scope API is
also unknown, not a purity proof. Its no-panic refusal control retains this
limit without changing the shared-side-effect helper or family accounting.

Runtime-name regression work (#122/#124) preserves the census: checker startup
has three clean pinned seed controls, and independent TLC-order validation
refuses namespace history it cannot establish from a witness. Canonical
validation remains decisive. These repairs do not certify their containing
families or change the 37-family denominator.
## Native DFID evidence

`dive dfid` and `dfid_file` implement the bounded pure safety visitation path
(ADR 0021). The [control report](../conformance/reports/dfid-primary-controls.json)
accounts every clean selected observation; worker-selection ambiguity is not
converted into strict agreement. Separately identified injected-worker-seed
references check status/iteration/counting and concrete path selection. The
complete acceptance matrix, output/error/display controls, effects, checkpointing
and other mode interactions remain pending. F-025/F-028/F-035 are partial and
audit-required; no denominator or certification count is changed.


### Retained temporal expression arguments (P3-scopes, #127)

ADR 0023 adds pure temporal expression forwarding through direct/local calls and
caller expressions holding nested INSTANCE bodies. Both planner and independent
periodic interpreter retain the actual scope; static formal levels do not
classify a forwarded temporal argument as a value atom. Supported ENABLED/WF
word controls preserve the independent finite cover's existing limits.

The fourteen clean raw probes retain ten temporal executions, two exported
INSTANCE reference refusals and two state-only safety classifications. Native
checks cover 120 final/short profile/store/requested-width combinations, 24
INSTANCE refusals, four safety classifications, and 1,960 Boolean word/closing
edge/profile/property controls. Fresh whole-trace negatives reject bad Init,
unknown claims and invalid loop indices. The original scope delivery captured inline; current integration checks the actual requested pool width at 1/4/8 workers.

F-006/F-007/F-019/F-021 remain partial/audit-required. Recursive local-function
self-binding and branch-sensitive finite preflight, higher-order/operator
arguments and wider substituted ENABLED covers still need proof. These controls
are not strict TLC tableau, timing, error-class or lasso conformance.

`conformance/studies/temporal-scopes-53-map.json` preserves all 53 original rows
with source-report/binary hashes and exact old outcomes/details: 26 final/short
matches, 22 refusals and five timeouts. It is a read-only blocker map, not a new
run. Neither that report nor the ten-model domain report gained match credit.


Unchecked Rust expression references and retained lazy expression IDs now
produce typed refusals at the planner/periodic endpoints, including missing
scoped definitions. Missing references have no invented source span. The raw
native `native-unchecked-context-before.*` repro in the probe directory records
the prior panics; it is not an oracle result. Constructing an evaluator with an
invalid redirect target can still panic before these endpoints run (#129).
General caller-created IR/context validation remains a separate partial API
obligation; this increment does not redesign the evaluator constructor.

### Bounded TLC normalization and pre-tableau grouping (P3-planning, #130)

ADR 0025 adds a diagnostic projection retaining predicate occurrence tags,
negation/simplification/DNF stages, TLC's recurrence-disjunction collection,
EA/AE classification, and ordered OOS/PEM bins. The model entry point retains
configured property identities and existing scope/purity checks; its private
WF shape matches pinned `astToLive`. Existing kernel/checker behavior is
unchanged.

All 84 supplied-formula controls and twelve real FastTool models remain in the
selection. Completed field projections, classifier/semantic refusals and DNF
cardinality failures are accounted separately. Actual processLiveness arrays
check the development adapter; independent scoped periodic words test every
transformation and reconstructed grouping. Existing historical corpus reports
are immutable and receive no new matches from these controls.

F-019/F-020/F-021 remain partial/audit-required. Binary/tableau/promise identity,
phase-L retention, periodic timing, SCC acceptance and TLC witness selection
are not implemented by this stage. Strict integrated liveness stays refused;
planning success must never be interpreted as proof or certification.

`TpLocal` distinguishes pinned Action/configured name `Live` from the derived
`getImpliedTemporalNames()` value `Identity`. Dive's role table matches that
latter field, while the configured-property table retains `Live`. All eleven
completed controls match those separate fields. Retaining action attribution
through future tableau/witness execution remains a separate representation
obligation (#134); grouping equality does not certify property attribution.

### Bounded TLC particle/tableau identities (#136)

ADR 0027 adds a separate diagnostic stage for balanced binary syntax, ordered
particle closure/representatives, initial particles, first-encounter graph
numbering, duplicate edges, state predicates and ordered promises/fulfillment.
The 137-row complete selection retains 126 exact completed projections, five
normalization failures, four direct structural/action refusals and two SANY
refusals. Both invalid and corrected INSTANCE controls and the complete initial
136-row snapshot remain auditable. Independent periodic/scoped word checks and
fresh whole-witness mutants cover truth and closing edges. Original
configured/Action names remain separate from derived temporal role labels.

F-019/F-020/F-021/F-022 remain partial/audit-required. This artifact does not
implement phase-L retention/timing, strict graph/SCC execution, witness selection
or attribution through execution; existing strict liveness refusals are intact.
No historical corpus outcomes receive extra matches from these diagnostics.



Bounded sequential PlusCal translation now has a separate
[artifact and generated-model matrix](pluscal-translation.md); F-034 remains
partial and audit-required. Source AST acceptance, generated token/operator
structure, module validity and finite model/witness behavior are separately
accounted. None certifies pending process/procedure/macro/option/rewrite/map cases.

Public Rust admission (#129, ADR 0028) now rejects malformed evaluator maps,
expression/operator IDs, redirect cycles/signatures and foreign captured
contexts before effects. Trace/planner/enabledness entries check the bound
model/evaluator pair before certificate work. Native regressions and the
original constructor panic are retained in
`crates/dive-api/tests/fixtures/public-context/`; this is additional F-035 evidence,
not a status upgrade or a full semantic-annotation/provenance certification.

The [sequential normalization increment](pluscal-normalization.md) adds an owned
110-row label/initializer/modifier selection and 16 generated finite-model
controls. F-034/F-035 remain partial and audit-required; the 37-family census
and certification denominator are unchanged.

### Bounded concrete phase-L diagnostic receipts (#140)

ADR 0029 adds real concrete successor collection with internally derived TLC
fingerprints, first representatives, grow-before-duplicate slot order and one
logical multiplier history. Null-tableau OOS action bins precede state bins;
initial tableau consistency is separately projected. Actual diagnostic execution
is inline (one worker); wider requests are refused honestly. Every receipt says
`graph_insertion=false`, and no receipt is proof evidence.

The complete `conformance/probes/tlc-phase-l/` selections retain 55 structural
controls, 35 actual synthetic LiveCheck controls, 51 bound model rows and 20 clean
TLC runs. Completed field/error projections, invalid syntax, effects/introspection/
state-IF refusals and tableau graph-pending rows are accounted separately. The
prior planning IF mismatch (#147) and body-first nonstuttering mismatch (#149)
remain available; conservative diagnostic refusal is not a match. Independent
trace controls reject forged Next/closing edges and claims. Previous corpus
reports are immutable and receive no new verdict credit.

F-020/F-021/F-022 remain partial/audit-required and F-023 missing. #140 stays
open: graph/product insertion, actual parallel commits, additional consistency/
error ordering, SCC/lasso selection and scheduling must still be implemented.

### Conditional planning error precedence (#147)

The TLC-shaped planner now preserves eager IF/boolean conversion errors and
nonrecursive-call fallback, with typed TLC errors and original source context.
All24 selected cases include two pre-planning SANY rejections. The original18
and every development failure remain visible. Actual state-space counts and
strict integrated temporal results receive no new credit; F-019 stays partial
and audit-required. See ADR0036 and the owned state-if-planning probes.

PlusCal process evidence now includes [a bounded single-process increment](pluscal-processes.md):
242 retained rows and 26 finite model controls with independent witnesses. This
does not certify F-034 or F-035; multiple blocks, fairness, procedures/macros and
full option/map contracts remain pending.
Native F-032 evidence is a bounded pure safety snapshot/resume slice, with actual
saved frontier recovery, concrete predecessor identity, ordered names and strict
archive admission. It has no Java-byte interoperability or independent snapshot
certificate. Graph/liveness/runtime/spill recovery remain pending; see
[native-checkpoint.md](native-checkpoint.md). No family certification is added.


The [bounded process-fairness increment](pluscal-fairness.md) adds weak/strong
declarations, label guards and explicit pinned fairness options. Translation
artifacts remain nonproof results. Multiple blocks, procedure fairness, renaming
and unproved domain normalization are still named refusals; full-family
certification remains pending.

Public backend admission (#157) adds checked no-work metadata and shared
pre-effect IR admission for ordinary/native checking plus bound graph capture.
Foreign/malformed predicate/action contexts refuse before assumptions; valid
metadata and exploration controls remain unchanged. The original panic and
native controls are retained in `conformance/probes/public-ir-admission/`.
This is additional F-035 evidence, not a status/certification upgrade.

Bounded [multiple finite-process translation](pluscal-multiple-processes.md) retains
264 C/P rows: 206 artifacts with enclosing-module operator CST/SANY agreement,
10 matching rejections and 48 named refusals. Sixty finite generated-model controls
and independently validated/corrupted witnesses cover the admitted storage/action
subset. F-034 and F-035 remain partial and audit-required; this does not certify
full process renaming, procedures/macros, historical options or file rewriting.



F-032 additionally covers pure supported VIEW/safety-symmetry recovery (ADR 0038).
The primary pinned minimal models and separate native recovery matrix preserve
concrete witnesses while rebuilding configured reduced visited keys. Equivalent
keys never substitute for independent Init/Next validation. This adds staged
evidence only: F-032 remains partial/audit-required with broader recovery pending.

### Native PlusCal translation increments

The [bounded native macro increment](pluscal-macros.md) adds simultaneous lexical
substitution, selector tails, earlier nested declarations and original definition/
call locations. Source/parser/normalization/raw generation/module admission
remain separate: invalid primary artifacts never count as admitted matches.
Unproved binders/procedures/name rewrites/options/maps remain pending; F-034/F-035
stay partial and audit-required and the certification denominator is unchanged.


The [sequential procedure study](pluscal-procedures.md) retains 84 C/P rows:
48 admitted artifacts, 14 matching rejections, 20 named refusals and two diagnostic
invalid-reference dispositions. All four primary SANY-invalid outputs stay
uncredited. Forty-six complete generated models match TLC; thirty invariant
witnesses have matched exploration but pending strict encounter-order validation.
Separately qualified default replay validates them and rejects 120 mutants without
conferring TLC certification. F-034/F-035 remain partial and audit-required.

The historical [nonrecursive procedure graph increment](pluscal-procedure-graphs.md) retains
120 capture-first C/P rows: 68 admitted artifacts, 19 source rejections, 29 named
refusals and four diagnostic invalid-reference dispositions. All eight primary
SANY-invalid outputs stay uncredited. Cycles, scopes/renaming, process stacks,
options/maps and strict encounter-order/deadlock evidence remain pending.
F-034/F-035 remain partial and audit-required; no denominator change.

The native disjoint sequential recursive procedure increment (ADR 0043) retains
all 116 C/P source cases: 86 artifact matches, 15 matching rejections, nine named
refusals and six diagnostic invalid-reference dispositions. All ten SANY-invalid
primary outputs remain uncredited. Enclosing SANY/CST, finite execution and
strict pending versus qualified canonical witnesses are separate contracts.
Unbounded-model primary timeouts and explicit native capture bounds never earn
proof credit. F-034/F-035 remain partial/audit-required; see
[recursive procedures](pluscal-recursive-procedures.md).

### D-013 historical increment: ordered temporal execution

The earlier staged source records this scope; the later explicit final CLI
contract is described in [the current user guide](tlc-final-api.md).

The bounded Rust execution path now commits real multiple-OOS/tableau graphs in
source order, with initial consistency before initial invariants and immediate
done-state re-enumeration. Fifteen completed pinned graph-field cases match at
1/3/4/8 workers; all 26 cases and eight clean TLC stopping controls are retained
in `conformance/probes/tlc-phase-execution/`. Logical prefix counters, speculative
batch emissions and recursive enumeration work remain separate. Independent
validation rejects altered words, closing edges and incomplete property claims.

This remains intermediate: accepting-sink witnesses, exact TLC SCC/lasso choice,
initial error replay and periodic checking are not certified. Ordinary strict
liveness stays refused; no defaults or fixed budgets increase. F-020/F-021/F-022
remain partial/audit-required, F-023 missing and #140 open. Previous complete
studies keep their original outcomes and receive no new corpus verdict credit.

### Bounded final-only historical library increment: pinned SCC and witness selection

This section preserves the first library producer and its selected controls.
The later CLI exposure does not retroactively change that receipt.

The bounded `dive_api::tlc_temporal` Rust mode now constructs its own ordered
graph and reproduces TLC's deciding component, greedy postfix and fingerprint-
bucket closure. It retains configured claims separately from derived role names.
At 1/3/4/8 workers, 19 completed pinned controls match their SCC/witness fields;
all 30 controls, including 11 uncredited failures, remain available in
`conformance/probes/tlc-final-check/`. Fresh independent validation accepts the
supported original words and rejects changed Init, Next/closing edges, loop
targets and incomplete claims. A 4,096-graph independent census checks acceptance
and selected path labels.

This is an explicit final-only TLC/fingerprint library mode, not ordinary strict
CLI parity or periodic-schedule support. Exact-store temporal proof is refused:
a fingerprint graph must not silently merge concrete states while claiming exact
identity. Accepting-sink reconstruction, initial error replay, symmetry/VIEW/ALIAS,
periodic hooks, unsupported scopes/effects and fixed resource bounds remain named
limitations. Holds results retain collision qualifications; backend candidates
never carry their own validation certificate. F-019 through F-022 remain partial/
audit-required, F-023 missing and #140 open. Old broad studies retain their original
outcomes; no default or ordinary strict conformance credit changes.

See [final-only Rust API guide](tlc-final-api.md) for examples, resolved schedule,
actual evidence and refusal boundaries. The API admits startup sources before
binding creates a host; effectful ASSUMEs, unused top-level constants, config
replacement candidates and hooks return named refusal with zero used workers.
Standard-module classification does not bypass this admission. Explicit sequential
requests/fallback execution remain refused until their observer path is certified.

The [single finite process procedure increment](pluscal-process-procedures.md)
retains 112 capture-first C/P controls and owned parameter/local/stack/pc
transfers. Native artifacts and generated-model/witness stages remain separate;
all invalid primary output is uncredited. Multiple process/procedure ownership,
renaming/process-declared fairness and strict #124 namespace replay remain pending.
The separate two-row algorithm-level fair study proves global WF_vars(Next)
artifact generation with CST/SANY and four finite Init/Next controls; it does
not certify liveness execution. F-034/F-035
stay partial/audit-required; no family denominator or certification change.

The [shared finite-process procedure increment](pluscal-multiple-procedures.md)
retains a separate complete 166-row C/P selection. One procedure normalization
feeds all owned process bodies; global quotas and constant/procedure/member
scheduling are source-backed. Invalid primary outputs and named scope/fairness
refusals remain visible. Artifact, finite execution, strict namespace-pending
and qualified canonical witness evidence stay separate. F-034/F-035 remain
partial and audit-required, with no family denominator change.

### Bounded process-owned procedure fairness

The [fair procedure guide](pluscal-procedure-fairness.md) extends the native
finite, disjoint caller path with per-caller transitive WF/SF and label modifiers.
It preserves 328 original cases, 146 separately corrected modifier cases and six
separate timeout follow-ups. These stages do not certify full F-034/F-035:
SANY-invalid primary artifacts remain uncredited, fair Spec generation is
nonproof, and strict #124 witness replay stays separate from qualified canonical
replay. Scoped renaming, sequential procedure modifier interactions and full
historical option/format/map contracts remain pending.


F-032 pure finite safety recovery additionally resumes the saved level through
the deterministic worker pool, with width/chaos and cross-width concrete-result
controls and inline-equivalent native counter stopping prefixes. This remains
staged evidence: runtime/temporal/graph/spill recovery, broader persistence
interactions and full certification remain pending.

### Bounded Full PlusCal checksum derivation

[Block rewriting](pluscal-rewriting.md) now supports Full requests for a proved
bounded sequential post-translation AST subset. Native serialized bytes are
compared with corresponding clean primary AST captures; parsed/name-fixed hashes
are separate development evidence. Full CRC derivation, literal supplied-body
editing bytes, SANY/Validator artifact admission and finite model/witness checks
remain separate dispositions. The `--fair` CRC weakness is reproduced, flagged
and never treated as source identity. Unproved AST shapes/name fixes, exact
generator formatting/file writes/maps and full F-034/F-035 certification remain
pending; both families stay partial/audit-required. The complete study is in
`crates/dive-pluscal/ast-checksum-acceptance/`, including initial failed controls.

### Nesting and expression-layout checksum continuation

The raw serializer accounts for all 320 retained sources: 224 exact pinned
post-translation AST matches, 14 named refusals and 82 primary rejections.
`elsif` and multiple-With generation admission is separate from multiline AST
derivation, whose native generator still refuses by name. Empty-Else TRUE
fidelity retains the original operator-CST failure and paired controls.
See [the runnable scope guide](pluscal-checksum-expressions.md) and
`crates/dive-pluscal/checksum-expression-acceptance/`. The family remains
partial/audit-required; no source-integrity or complete translator certification.

Bounded sequential non-junction multiline PlusCal generation is now tracked in
`docs/pluscal-multiline.md` and its complete capture. Raw AST CRC, generated
operator trees/admission and finite model/witness evidence remain separate.
F-034/F-035 remain partial and audit-required; all unproved contexts refuse.

Explicit finite-process fairness/automatic-label Full checksum settings now have
separate pinned parser/name/translated AST, public module admission, configured
Validator and finite safety/witness evidence. Source option comments remain
named unsupported after retained ignored-option failures; unconfigured reference
validation is qualified as D-021, with D-020 unchanged. See
[the runnable CLI/API contract](pluscal-process-checksum-options.md).
F-034/F-035 remain partial and audit-required; no denominator/certification change.

Source-option context continuation (#182) uses the shared bounded first-declaration
resolver for generation and Full checksum serialization, preserving requested and
effective settings plus original spans. Unknown/malformed/conflicting option
contexts and old-block context masking refuse explicitly. Native AST/CRC/CST,
public admission, Validator and finite-model evidence remain separate. See
[the runnable source options guide](pluscal-source-options.md). F-034/F-035 remain
partial and audit-required; no certification or denominator change.
