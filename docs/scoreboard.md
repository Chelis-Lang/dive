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

### Accepted Bits baseline

The current accepted Bits tree is `f5c999ff7b91689658eb6c9e55488300386d7598`,
measured at producer `5f9e11505eda76020b9cbd03f2f4ceae6262a937` and merged unchanged
as `bd8804e8681a1d2c60a62abd58d32ac692512c77` in
[PR #286](https://github.com/Chelis-Lang/dive/pull/286). The four actual native
campaigns retain the complete original 154 IDs, pins, goldens and 600-second bounds:

| Date | Profile | Workers | Entries | Agree | Unsupported | Expected divergence | Disagree / timeout / pending / oracle error |
|------|---------|--------:|--------:|------:|------------:|--------------------:|-------------------------------------------:|
| 2026-10-15 | TLC | 1 | 154 | 129 | 25 | 0 | 0 |
| 2026-10-15 | TLC | 4 | 154 | 129 | 25 | 0 | 0 |
| 2026-10-15 | default | 1 | 154 | 92 | 58 | 4 | 0 |
| 2026-10-15 | default | 4 | 154 | 93 | 58 | 3 | 0 |

The producer passed the full local gate: 1,871 Rust tests with no skips,
91 doctests, 456 SANY comparisons, and CM1/validation-smoke at 1/4/8 workers.
All three required source and post-merge main CI jobs passed, including the
actual cargo-deny step for the changed lockfile (main run `38709617664`).
Both native widths use the maintained one-worker-reference comparator. Historical
actual TLC4 Hanoi counts/depth remain different and receive no exact TLC4-match
credit. Issue #272's broader Bits gap and complete temporal parity remain open.
The configured feature's two added Reply IDs are separate fixtures and add no
real-model credit to these legacy totals.

### Historical corrected small-tier baseline

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

## Runtime Print extension

Ordinary finite one-worker simulation supports direct, captured and configured
Print/PrintT with separate startup/runtime output and quiet independent word
validation. Reached random/register/I/O effects remain named refusals; M/#239,
generation and schedulers remain partial. Print qualification on the latest main,
fresh review, full gate and publication are pending; the accepted statistics
measurements above are not retagged onto this candidate. See `docs/user/simulation.md`.

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

## Current-parent ordinary simulation Print acceptance

The accepted scope is bounded Print/PrintT runtime capture, private replay output
discard and shared-core simulation ownership. It adds no complete-state-space or
expression coverage claim. Historical pending/rejected-cut qualifications retain
their original source identities; the current-parent measurement is below.

Runtime source `2fa57f734adf1a065555d7ab18f40b1cd2ca416b`, tree `24f070f2b326d5a6526c8342b09f471e62179b21`, directly based on actual accepted API main `4b6b6fd4a6bb1e0c69a9b0abfce61c52a0237571`.

Its actual full gate passed 1,745 workspace tests with no skips, 86 doctests and 456 SANY comparisons, with CM1 and validation-smoke at 1/4/8. All four original 154-ID TLC/default 1/4-worker reports retain original pins, goldens, 600-second bounds and per-ID no-loss obligations.

Current-parent scoped execution passed 73 tests with no failures, ignores or filtering and recorded 36 JSON CLI cases; the separate human controls and fresh review keep their actual receipt bindings.

| Profile | Workers | Original IDs | Agree | Refused | Expected divergence | Disagree | Timeout |
|---|---:|---:|---:|---:|---:|---:|---:|
| TLC | 1 | 154 | 95 | 59 | 0 | 0 | 0 |
| TLC | 4 | 154 | 95 | 59 | 0 | 0 | 0 |
| default | 1 | 154 | 91 | 59 | 4 | 0 | 0 |
| default | 4 | 154 | 91 | 59 | 4 | 0 | 0 |

Historical failed overlap, rejected cuts and invalid experiments remain preserved and uncredited. The later two-Markdown publication transfer does not retag runtime measurements. Actual source/main CI is a separate obligation. Full M/#239, generation, schedulers, distribution/DFID and general coverage remain open; sampling is no safety/liveness proof.

## Bounded SVG PointOnLine candidate

This separate transfer candidate adds the pinned numeric helper for literal
stored signed32 coordinate records. Historical primary and native controls retain
their recorded revisions. The direct24b499 `bc070808` slice passed bounded
scoped controls and independent review; it is not current-parent gate credit.
Current-source transfer qualification, fresh review, full gate, unchanged
complete corpus matrices and publication remain pending. No
scoreboard totals change. Computed/deferred representations and unimplemented
JVM errors remain named refusals. SVG serialization, graph/ring layout, animation
host effects and full CommunityModules/Examples/VTS parity remain open.

## Current-parent bounded SVG PointOnLine acceptance

The accepted scope is the bounded numeric helper for stored coordinate records.
The current-source finite controls include numeric/error/deferred/resource and
provider routing, Point/Print same-Core cleanup and independent word validation.
Earlier invalid and historical measurements keep their original identities.

Runtime source `d40c12e626a0c300faeea20adebc1ba9a5f0ec97`, tree `045ccf994a7aadad2ae7f121487a1bab258d88fe`, directly based on actual accepted Print main `952c84bb38e0c044da5188c03712c3758604f219`.

Its actual full gate passed 1,753 workspace tests with no skips, 86 doctests and 456 SANY comparisons, with CM1 and validation-smoke at 1/4/8. All four original 154-ID TLC/default 1/4-worker reports retain original pins, goldens, 600-second bounds and per-ID no-loss obligations.

Current-parent finite scoped execution passed 76 tests with no failures, ignores or filtering and recorded 36 JSON CLI cases; all three external probes and the genuinely fresh independent review keep their actual receipt bindings.

| Profile | Workers | Original IDs | Agree | Refused | Expected divergence | Disagree | Timeout |
|---|---:|---:|---:|---:|---:|---:|---:|
| TLC | 1 | 154 | 95 | 59 | 0 | 0 | 0 |
| TLC | 4 | 154 | 95 | 59 | 0 | 0 | 0 |
| default | 1 | 154 | 91 | 59 | 4 | 0 | 0 |
| default | 4 | 154 | 91 | 59 | 4 | 0 | 0 |

Historical compile/parser failures, invalid compound-initial fixture and review build attempts remain preserved and uncredited. The later two-Markdown publication transfer does not retag runtime measurements. Actual source/main CI is a separate obligation. Full SVG/C, layout, serialization, animation and general CommunityModules parity remain open; issue #272 remains open.

## Current-parent configured safety-role checking acceptance

The accepted scope is the narrow default-memory final/short safety-role route.
Finite controls retain scopes/refinements, first errors, startup output/RNG,
option exclusions, recursive state evaluation and genuine temporal validation.
Current SVG/Print/API integration and independent good/forged words are retained.
Earlier pending/historical notes keep their original measurement identities.

Runtime source `f939f4bfdcf131b94a259bd445655e206d9a4c4f`, tree `a5235897aee36ddbfe8182d1acae528bfd256aec`, directly based on actual accepted SVG main `69cfb8b0d0bcd2721157331d792e58539cf52072`.

Its actual full gate passed 1,767 workspace tests with no skips, 86 doctests and 456 SANY comparisons, with CM1 and validation-smoke at 1/4/8. All four original 154-ID TLC/default 1/4-worker reports retain original pins, goldens, 600-second bounds and per-ID no-loss obligations.

Current-parent finite execution passed 140 selected tests with no failures, ignores or unexecuted selections; 142 other tests are explicitly unselected. It recorded 38 expected-status Print CLI controls and 32 doctests. Fresh independent review passed the same selected population, 4 additional Rust probes and 24 supplemental CLI commands. Failed diagnostic/helper attempts remain their actual negatives.

| Profile | Workers | Original IDs | Agree | Refused | Expected divergence | Disagree | Timeout |
|---|---:|---:|---:|---:|---:|---:|---:|
| TLC | 1 | 154 | 95 | 59 | 0 | 0 | 0 |
| TLC | 4 | 154 | 95 | 59 | 0 | 0 | 0 |
| default | 1 | 154 | 91 | 59 | 4 | 0 | 0 |
| default | 4 | 154 | 91 | 59 | 4 | 0 | 0 |

Historical source/runtime failures, invalid external helpers and review experiments remain preserved and uncredited. This later two-Markdown transfer does not retag measurements; actual source/main CI is separately verified. General API, temporal and TLC parity, broad P/C/M completion and issue #272 remain open; no state-space gain or roadmap completion follows from this bounded change.

### Ordinary TLC-profile temporal boundary

Eligible ordinary TLC-profile requests use the existing bounded pure final
backend and independently validated short periodic words. Safety-only bound
roles retain ordinary schema-1 results; actual temporal analysis uses schema 2
with complete capture counts. Native ordinary temporal checking, excluded
options and unsupported language combinations retain named refusals. Periodic
scheduling and general temporal parity remain incomplete. This source boundary
adds no measured corpus count; existing measurements retain their producer
identities. See [the temporal guide](user/temporal.md#ordinary-checking-in-the-tlc-profile).

PlusCal option coverage includes measured termination/fairness defaults,
noDoneDisjunct, widths, label reporting and sequential version behavior with
shared effective generation options. Backup/cfg/EOL flags expose in-memory
preferences. Original-stage bytes have bounded native operations with measured
original procedure/macro and compound ownership. Specification-backed translators,
pre-1.5 process/procedure shapes and filesystem writes remain unfinished; full P
and its primary TLC-versus-native model-checking milestone are still partial.

PlusCal checksum retirement keeps real generated-body CRCs and uses optional exact
procedure-free algorithm CRCs. Unavailable procedure/header CRCs never gate actual
translation, rewriting or debug generation. Schema2 removes proof-only reporting;
current full149/native-versus-primary model checking remains unmeasured and P
remains partial. Unlike headers do not receive exact-byte agreement credit.

## Historical pinned local Bits native reference candidate

Three retained directTLC4 Hanoi runs report243distinct/907generated/depth33;
historical native4 and the primary/golden1 report233/768/depth32, with the same
NotSolved/2110/physical12 and valid32state native words. All raw observations and
negative comparisons remain historical and are not regraded. The former local-
class one-worker restriction fenced that stopping-prefix discrepancy. The
existing comparator uses the one-worker reference for native implementations
at every width; only TLC oracle self-checks have multiworker leniency.

The proposed revised checker follows the maintained native contract: every
supported native width compares to the pinned one-worker reference. Required
verdict/property/class/physicalexit, counts/depth and independent validation of
the actual concrete native word remain unchanged. This removes only the local
class width guard; exact class admission and checkpoint class-source identity
stay binding. General store/effect/capture, simulation and DFID scopes retain
existing restrictions. No comparator, golden, ExpectedDivergence, queue-order or
timing exemption changes.

Acceptance requires latest-parent original-MC1 qualification, a separate
complete-search primary/native1 diagnostic, supported native4 pinned-reference
and independently validated actual-word controls, fresh review/fullgate/all4/CI.
Issue #272 remains open; this source proposal is not runtime acceptance.

No existing corpus totals/floors change. General Java support and full Bits/C
parity remain incomplete. Original negative/invalid attempts remain retained.

Direct configured first-order user operators in retained pure temporal atoms
have two dedicated pinned Reply probes under
`conformance/corpus/configured-temporal-operators`: success and a named temporal
violation, each with 2 distinct/3 generated states and depth 2. Captured,
substituted, recursive and native configured targets retain refusals. This
source increment adds no measured real-corpus agreement or phase completion;
older census rows retain their original producer identities.

## Finite recursive-function scope candidate

Local pure state RecFcn contexts now retain real self/domain/formal, lexical and
invocation/state bindings through temporal planning, finite execution and
independent temporal/EXISTS inspection. Seven tiny primary cases precede source
implementation; they do not add original allocator/corpus agreements. Configured,
higher-order/opaque and primed/effectful recursive captures remain refused, with
existing resource and termination limits. Current-source qualification, fresh
review and full gates remain pending. See `docs/user/temporal.md`.
