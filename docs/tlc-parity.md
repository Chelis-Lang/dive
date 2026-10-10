# TLC parity ledger

Baseline: main `217cebfa8bd35735bf14d38f28a4ae17628739aa`, 2026-10-10.

The [delivery plan](design/tlc-parity-plan.md) covers the entire pinned TLC
capability set through a native Rust checker, including simulation, DFID,
distribution, recovery and PlusCal. This ledger records the denominator and
evidence without converting partial implementations into completed features.

## What the denominator means

The maintained [semantic ledger](../conformance/parity-features.json) has **37
stable feature families**: seven have an implemented core, twenty are partial,
and ten are missing. Those implementation labels do not certify full parity.
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

## Semantic feature families

Each JSON row owns source/Rust references, acceptance criteria, interactions,
existing issues and its phase in the delivery plan. `implemented` describes
the core path; it does not remove the listed certification obligations.

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
| F-017 | Symmetry declaration binding, warnings and closure | partial | 2 | [symmetry_declaration.rs](../crates/dive-model/tests/symmetry_declaration.rs) |
| F-018 | Symmetry reduction, concrete traces and audit | missing | 2 | [m6-plan.md](design/m6-plan.md) |
| F-019 | Temporal language, simplification and exact tableaux | partial | 3 | [planning.rs](../crates/dive-liveness/tests/planning.rs) |
| F-020 | Temporal graph capture and phase-L timing | partial | 4 | [temporal-study.md](temporal-study.md) |
| F-021 | WF/SF fairness and independent periodic validation | partial | 4 | [temporal.rs](../crates/dive-api/tests/temporal.rs) |
| F-022 | SCC acceptance, TLC lasso selection and attribution | partial | 4 | [lassos.rs](../crates/dive-liveness/tests/lassos.rs) |
| F-023 | Periodic/final/off/sequential liveness schedules | missing | 4 | [m6-plan.md](design/m6-plan.md) |
| F-024 | Simulation, generation, scheduling and random replay | missing | 5 | [mod.rs](../crates/dive-stdlib/src/random/mod.rs) |
| F-025 | DFID checking | missing | 5 | [tlc-parity-plan.md](design/tlc-parity-plan.md) |
| F-026 | Granted environment, file and process effects | missing | 6 | [tlc-differences.md](tlc-differences.md) |
| F-027 | Runtime introspection and configuration services | partial | 6 | [tlc_module.rs](../crates/dive-check/tests/tlc_module.rs) |
| F-028 | CLI/output conventions and message controls | partial | 6 | [sprint-status.md](sprint-status.md) |
| F-029 | Trace/state dumps, ALIAS and trace exploration | partial | 6 | [trace_controls.rs](../crates/dive-api/tests/trace_controls.rs) |
| F-030 | Coverage and debugger services | missing | 6 | [tlc-parity-plan.md](design/tlc-parity-plan.md) |
| F-031 | Bounded-memory stores, frontiers, graphs and SCC spill | missing | 7 | [m5-plan.md](design/m5-plan.md) |
| F-032 | Native checkpoint and recovery | missing | 7 | [tlc-parity-plan.md](design/tlc-parity-plan.md) |
| F-033 | Native distributed checking and recovery | missing | 7 | [tlc-parity-plan.md](design/tlc-parity-plan.md) |
| F-034 | PlusCal translation and translation validation | missing | 8 | [frontend-campaign.md](frontend-campaign.md) |
| F-035 | Public Rust APIs and cargo-test integration | partial | 9 | [temporal.rs](../crates/dive-api/tests/temporal.rs) |
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
  exact lassos or parallel graph capture.
- **Frontend:** [the deep campaign](frontend-campaign.md) agrees on 1,108
  selected inputs in both direct grammar and full SANY checking, retaining
  the difference between those measurements. It is a bounded campaign.
- **ValidationTestSuite:** [the smoke import](conformance-corpora.md) accounts
  for all 36 selected cases and provides 16 generated models at 1/4/8
  workers. The entire source suite and TLC JUnit model corpus remain open
  work under #2.
- **Symmetry:** the declaration/closure tests provide early binding and
  1,329 direct Java closure controls. Ordinary SYMMETRY checking is still
  explicitly refused; canonical reduction and trace reconstruction are missing.

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
| `-simulate`, `-generate`, `-depth`, `-dfid` | F-024/F-025; simulation and DFID backends missing |
| `-checkpoint`, `-recover`, `-metadir`, `-cleanup` | F-031/F-032; native persistence/recovery missing |
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
All belong to F-034, whose backend remains a placeholder. Commented-out
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
unknown claims and invalid loop indices. Capture still uses one worker.

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
