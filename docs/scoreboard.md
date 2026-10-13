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

The lead updates these numbers from complete harness reports. The latest
measurement is the small tier at producer commit
`5491852659f9a01c45279c9817cda24bedbf15d6`, merged with an identical tree as
`ea94776032cac390749a71d764433918bcfb641d` in
[PR #260](https://github.com/Chelis-Lang/dive/pull/260). Each run selected the
same 154 example, probe and validation-smoke models with unchanged pins,
goldens and bounds.

| Date | Profile | Workers | Entries | Agree | Pending validation | Unsupported | Expected divergence | Disagree | Timeout | Oracle error |
|------|---------|--------:|--------:|------:|-------------------:|------------:|--------------------:|---------:|--------:|-------------:|
| 2026-10-13 | TLC | 1 | 154 | 91 | 3 | 60 | 0 | 0 | 0 | 0 |
| 2026-10-13 | TLC | 4 | 154 | 92 | 2 | 60 | 0 | 0 | 0 | 0 |
| 2026-10-13 | default | 1 | 154 | 87 | 3 | 60 | 4 | 0 | 0 | 0 |
| 2026-10-13 | default | 4 | 154 | 87 | 3 | 60 | 4 | 0 | 0 | 0 |

The full local gate passed, including all 1,551 workspace tests, CM1 and
validation-smoke at 1/4/8 workers, and 456 SANY module comparisons. These
small-tier measurements do not establish full-tier agreement or completion
of a roadmap feature.

The original TLC 93/default 90 floors remain unmet. Only the three named
agreement-to-pending transitions in
[ADR 0093](decisions/0093-temporary-validator-parity-exception.md) are
temporarily accepted: DieHardest, P5TLCGetSet and P9Random. Pending validation
earns no agreement credit. The four-worker TLC DieHardest run ends at its
ASSUME failure, so it needs no counterexample validation. Startup-register
and random-membership support must recover agreement through independent
validation and fresh corpus runs; scoped tests alone do not change this table.

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
