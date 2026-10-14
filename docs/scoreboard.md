# Scoreboard

Parity means observable results (ADR 0086). For every corpus model, dive must
match the pinned TLC on:

- the verdict;
- distinct and generated state counts (at 1 worker);
- the violated property name;
- the error class and exit code;
- the depth;
- a counterexample that the independent trace validator accepts.

dive does not emulate arbitrary or nondeterministic TLC internals, such as
timing, thread or queue order, tableau numbering or SCC and lasso choice.

## Corpus scoreboard

The lead updates these numbers from complete harness reports. The corrected
small-tier baseline below uses producer
`643b0f1351feccca56968cd8032093f53ccd2c8d`, published with an identical tree as
`0e00c942b5ad371acadb9b805509ebceef313cd4` in
[PR #266](https://github.com/Chelis-Lang/dive/pull/266). All four runs retain the
same 154 models, pins, goldens and 600-second bounds.

| Date | Profile | Workers | Entries | Agree | Pending validation | Unsupported | Expected divergence | Disagree | Timeout | Oracle error |
|------|---------|--------:|--------:|------:|-------------------:|------------:|--------------------:|---------:|--------:|-------------:|
| 2026-10-14 | TLC | 1 | 154 | 94 | 0 | 60 | 0 | 0 | 0 | 0 |
| 2026-10-14 | TLC | 4 | 154 | 94 | 0 | 60 | 0 | 0 | 0 | 0 |
| 2026-10-14 | default | 1 | 154 | 76 | 0 | 60 | 18 | 0 | 0 | 0 |
| 2026-10-14 | default | 4 | 154 | 76 | 0 | 60 | 18 | 0 | 0 | 0 |

The exact source passed fresh independent review, the full local gate with
1,609 workspace tests and no skips, 80 doctests, CM1 and validation-smoke at
1/4/8, and 456 SANY comparisons. All three required source CI checks and all
three post-merge main checks passed. The complete corrected parent/current
matrix has eight actual measurements: seven fresh campaigns and the unchanged
candidate full-gate TLC1 report. Every per-model outcome is unchanged between
the corrected parent and candidate. This accepts the accounting correction;
it does not establish full-tier or at-least-95% parity.

### Historical small-tier measurement

The historical
small-tier reports below are from producer `e8c82c0e039f8da6308796a5bdf09ec5ab79a4fe`,
merged with an identical tree as `ade12658044bbc9195bae118351434b2082c4df9` in
[PR #263](https://github.com/Chelis-Lang/dive/pull/263). Every run retains the
same 154 example, probe and validation-smoke models, unchanged pins, goldens
and bounds.

| Date | Profile | Workers | Entries | Agree | Pending validation | Unsupported | Expected divergence | Disagree | Timeout | Oracle error |
|------|---------|--------:|--------:|------:|-------------------:|------------:|--------------------:|---------:|--------:|-------------:|
| 2026-10-13 | TLC | 1 | 154 | 94 | 0 | 60 | 0 | 0 | 0 | 0 |
| 2026-10-13 | TLC | 4 | 154 | 94 | 0 | 60 | 0 | 0 | 0 | 0 |
| 2026-10-13 | default | 1 | 154 | 90 | 0 | 60 | 4 | 0 | 0 | 0 |
| 2026-10-13 | default | 4 | 154 | 90 | 0 | 60 | 4 | 0 | 0 | 0 |

The full local gate passed all 1,599 workspace tests without skips, CM1 and
validation-smoke at 1/4/8 workers, and 456 SANY comparisons. All three required
source checks and all three post-merge main checks passed. The original
TLC 93/default 90 floors and both original and current-main per-model no-loss
comparisons were reported as passing under the old comparator. The default
agreement claim is invalidated by the omitted numeric-exit comparison described
below. Original floors and failed corrected comparisons remain visible; no
replacement floor is approved here. These historical measurements do not
establish current-main complete conformance, full-tier agreement or roadmap completion.

MCDieHardest, P5TLCGetSet and P9Random were reported as agreeing in all four runs;
their default safety rows require ExpectedDivergence for physical 12 → 10. Reviewed
private startup reconstruction and narrow direct initial random membership
recover the cases covered by
[ADR 0093](decisions/0093-temporary-validator-parity-exception.md); its temporary
acceptance exception is fulfilled and no longer needed. Historical failed
comparisons remain evidence of the earlier cuts. Pending validation still earns
no agreement. General random relations, independent action composition and
broader validator support remain open.

### Numeric-exit accounting correction

[ADR 0096](decisions/0096-truthful-default-numeric-exit-conformance.md) corrects
an omitted required observable. Independent diagnosis found 14 old false Agree
credits in each default report: twelve safety rows physically exited 10 against
TLC's 12, and M1Syntax/M2Undefined exited 1 against 150. The four old
ExpectedDivergence rows also omitted their exit differences. Exact pairs are now
registered on these measured model paths, retaining all other comparison and
independent-validation obligations. Native CLI/API behavior is unchanged.

The exit-only historical estimate was **76 Agree / 18 ExpectedDivergence /
60 Unsupported** at each default width. The actual corrected parent/current
measurements above now confirm those totals with all other comparison and
independent-word obligations enforced. The owner delegated judgment toward
eventual parity; the accepted measurement repair protects the actual corrected
parent's genuine agreement set and health. It does not approve a silent new
76 floor. The original 90/4/60 reports remain visible, alongside the original
T3 TLC93/default89 floors and the later TLC93/default90 floors. Corrected
comparisons against the historical default 90-credit reports fail because
fourteen credits omitted unequal exits. The TLC93 floor and all 94 corrected
parent agreements are preserved. Expected divergence never earns agreement.
The at-least-95% eventual parity target and whole roadmap remain open and unchanged.

### Historical full-tier measurement

The separately audited full-tier reports use source
`2edbb9bb45593016954540f28f2a9cdd1234ba38` and copied builder
`df64f21379a4131058094de76d440014b4b30967`, whose source trees are identical.
These totals also predate exact numeric-exit comparison and have not been
remeasured or audited here for exit-only disposition. This is a historical
native-harness measurement, not execution of current main
or additional independent TLC model truth. All five corpora and every selected
row are retained. The corrected bookkeeping audit is post-execution; original
rejected checkers and raw reports remain preserved.

| Profile | Workers | Selected models | Agree | Pending validation | Unsupported | Expected divergence | Oracle error | Out of scope | Disagree / timeout |
|---------|--------:|----------------:|------:|-------------------:|------------:|--------------------:|-------------:|-------------:|-------------------:|
| TLC | 1 | 263 | 107 | 3 | 61 | 0 | 44 | 48 | 0 / 0 |
| TLC | 4 | 263 | 108 | 2 | 61 | 0 | 44 | 48 | 0 / 0 |
| default | 1 | 263 | 103 | 3 | 61 | 4 | 44 | 48 | 0 / 0 |
| default | 4 | 263 | 103 | 3 | 61 | 4 | 44 | 48 | 0 / 0 |

Each report contains 1,342 modules: 1,341 agree and one is out of scope. The
44 in-scope models without generated goldens remain OracleError with unknown
results. Every actual harness exit is 1, and the full-tier gate remains false.
Refusals, pending validation, exclusions and missing goldens receive no agreement
credit. Current-main full-tier measurements and the at-least-95% goal remain open.

The historical 2026-10-09 small-tier measurement was 83 agree, 71 unsupported
and zero disagreements out of 154 models.

`unsupported` is an explicit, named refusal. It is never counted as agreement.
Any disagreement becomes an issue with a minimal reproducing spec.

## Roadmap feature checklist

Status is one word: `implemented`, `partial` or `missing`. `implemented` means
the feature works for the supported corpus. It does not certify full parity.
`conformance/parity-surfaces.json` is the reference inventory of TLC and SANY
surfaces.

| Family | Status |
|--------|--------|
| TLA+ grammar and lossless parsing | implemented |
| Semantic resolution, levels and module/proof syntax | implemented |
| Configuration parsing | implemented |
| Model/configuration binding and startup | partial |
| Values, equality, order, laziness and representation | partial |
| Evaluation scopes, recursion and higher-order operators | partial |
| Finite domains, successor enumeration and independent ENABLED | partial |
| Native override discovery and installation | partial |
| Standard-module operators | partial |
| CommunityModules operators and TLA+ fallback definitions | partial |
| Error classes and precedence | partial |
| Sequential BFS safety, invariants and deadlock | implemented |
| Deterministic parallel checking | implemented |
| Fingerprint/exact stores and collision accounting | implemented |
| State/action constraints and action-property split | implemented |
| VIEW identity and temporal lifting | partial |
| Symmetry declaration binding, warnings and closure | partial |
| Symmetry reduction, concrete traces and audit | partial |
| Temporal language, simplification and tableaux | partial |
| Temporal graph construction | partial |
| WF/SF fairness | partial |
| Liveness acceptance, counterexamples and attribution | partial |
| Periodic/final/off liveness schedules | missing |
| Simulation, generation and random replay | partial |
| DFID checking | partial |
| Granted environment, file and process effects | missing |
| Runtime introspection and configuration services | partial |
| CLI/output conventions and message controls | partial |
| Trace/state dumps, ALIAS and trace exploration | partial |
| Coverage and debugger services | missing |
| Bounded-memory stores, frontiers, graphs and spill | partial |
| Checkpoint and recovery | partial |
| Distributed checking and recovery | missing |
| PlusCal translation and translation validation | partial |
| Public Rust APIs and cargo-test integration | partial |
| Complete corpus, harness and release certification | partial |
| Global TLC profile and approved better defaults | partial |

## Basic simulation statistics

Finite one-worker simulation provides bound action inventories and optional basic
counts of consecutive chosen actions. These sample statistics do not prove
state-space or expression coverage. The full gate passed 1,716 tests with no
skips, 84 doctests and 456 SANY cases. All four unchanged 154-model runs preserve
the accepted baseline: TLC workers 1/4 each have 95 agreements and 59 refusals;
default workers 1/4 each have 91 agreements, four registered divergences and 59
refusals. No corpus agreement is added by this change. Simulation/generation and
DFID stay partial; schedulers and general coverage remain open.

## Checker exit-convention candidate

[ADR 0097](decisions/0097-check-exit-conventions.md) records the lead's new
explicit choice of TLC physical statuses for ordinary `dive check`, independent
of evaluator semantics, with native CLI/API access retained. The tables above
remain exact historical measurements. The conditional small-tier projection
90 Agree /4 ExpectedDivergence /60 Unsupported is not actual corpus evidence
or a replacement floor; fresh review, full gate, all four complete reports
and CI remain required. Expected divergence never earns agreement.

## Tuple/function storage regression coverage (#195)

Structural format 2 retains the concrete tuple/function distinction through
frontier spill and checkpoint/recovery. The consumed regression selection covers
application and independently validated invariant witnesses, plus comparison
controls, in both profiles/stores at workers 1/4/8 with plain/chaos scheduling.
Application detects the representation-dependent diagnostic; comparison and
independent mathematical witness validation cannot detect a lost representation
bit. Old value-bearing archives refuse; canonical keys, fingerprints and version-1 trace JSON retain
their separate contracts. This bounded coverage does not complete storage,
distribution or the full roadmap and adds no corpus agreement claim.

## Bounded pure Json

Bounded `Json!ToJson` and consumed fixtures are salvaged onto accepted main
`8036d335`, using the frozen historical implementation `1f21101a` as a reference.
Focused native/pinned-oracle controls and fresh independent review cover the
supported conversions and named refusals. The full local gate has 1,703 passing
tests, 83 passing doctests and 456 SANY agreements. Actual TLC-profile small
corpus runs at one and four workers each have 95 agreements and 59 refusals;
the JSON override probe moves from refusal to agreement with success, one
distinct state, two generated states and depth one. The other 94 agreements
preserve their observable results. The CommunityModules family remains partial.
Array/Object entry points, file operations, lost TLC object histories and
unbounded domain printing retain named refusals. Newly created JSON string names
in TLC-profile deciding words retain the existing independent replay refusal
(#124) and earn no agreement. Full Json/C parity remains open.

## Empty-property final-checking safety route

The bounded API/CLI repair in [#275](https://github.com/Chelis-Lang/dive/issues/275)
preserves ordinary safety behavior when there are literally no configured
`PROPERTY` entries and records actual binding/preparation model-error origin.
A configured state/action property is not an empty property list. No temporal
holding/capture proof or general liveness/API parity is credited on this route.

Runtime measurements use source `72ae013e8897b56eb76574ad3fb7a7293a7b53c3`, tree
`a5dbf925b8165bf7b6f345d55bf34359dc9a2c8a`, based on accepted main
`24b4994d78403b36bb716c6aca408370c8e7393f`. The actual full gate passed
1,726 workspace tests with zero skips, 84
doctests, CM1 and validation-smoke at 1/4/8, and 456 SANY comparisons.
The complete original 154-model matrix uses unchanged pins, goldens and
600-second bounds. The counts below are the observed runtime counts, not
measurements of the later documentation-only publication tree.

| Date | Profile | Workers | Entries | Agree | Pending validation | Unsupported | Expected divergence | Disagree | Timeout | Oracle error |
|------|---------|--------:|--------:|------:|-------------------:|------------:|--------------------:|---------:|--------:|-------------:|
| 2026-10-14 | TLC | 1 | 154 | 95 | 0 | 59 | 0 | 0 | 0 | 0 |
| 2026-10-14 | TLC | 4 | 154 | 95 | 0 | 59 | 0 | 0 | 0 | 0 |
| 2026-10-14 | default | 1 | 154 | 91 | 0 | 59 | 4 | 0 | 0 | 0 |
| 2026-10-14 | default | 4 | 154 | 91 | 0 | 59 | 4 | 0 | 0 | 0 |

All four comparisons preserve the accepted main TLC95/default91 agreement
floors and the original per-ID population and observables. No corpus agreement is added by this bounded routing repair.
Earlier measurements retain their original source labels, including the failed
`565bffe8` gate. Publication CI is a separate obligation; this bounded repair
does not establish full-tier agreement, at-least-95% parity or roadmap completion.
