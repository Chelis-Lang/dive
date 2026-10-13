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

Scoped ordinary completed-level consumer controls are separate from the corpus
measurements below. They do not update agree/refuse counts or certify phase completion;
full corpus/gate measurements remain required after integration (ADR 0094).

## Corpus scoreboard

The lead updates these numbers from complete harness reports. The current
small-tier measurement is producer `e8c82c0e039f8da6308796a5bdf09ec5ab79a4fe`,
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
comparisons pass in every run. These measurements do not establish full-tier
agreement or completion of a roadmap feature.

MCDieHardest, P5TLCGetSet and P9Random now agree in all four runs. Reviewed
private startup reconstruction and narrow direct initial random membership
recover the cases covered by
[ADR 0093](decisions/0093-temporary-validator-parity-exception.md); its temporary
acceptance exception is fulfilled and no longer needed. Historical failed
comparisons remain evidence of the earlier cuts. Pending validation still earns
no agreement. General random relations, independent action composition and
broader validator support remain open.

### Historical full-tier measurement

The separately audited full-tier reports use source
`2edbb9bb45593016954540f28f2a9cdd1234ba38` and copied builder
`df64f21379a4131058094de76d440014b4b30967`, whose source trees are identical.
This is a historical native-harness measurement, not execution of current main
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
| Periodic/final/off liveness schedules | partial: scoped completed-level/final consumer controls; off remains missing |
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
