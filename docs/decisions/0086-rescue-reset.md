# 0086: Rescue reset

- **Status:** accepted
- **Date:** 2026-10-12

## Context

By 2026-10-12, `main` tracked about 660,000 files. About 97% of them were
vendored evidence: probe captures, PlusCal acceptance trees, study dumps, logs,
compressed archives and full snapshot copies of the repository's own source.
Development had drifted from TLC's observable behaviour into reproducing TLC's
internals, for example its timing, queue service order, tableau numbering and SCC
and lasso choice. Each increment added an ADR, a doc, a checker script and an
evidence tree, and each increment's admission proofs had to be preserved. The
owner approved a rescue plan, tracked in issue #241. The roadmap is unchanged:
full TLC parity, including simulation, DFID, checkpoint/recovery, spill,
distribution, coverage/debugger, PlusCal and the public Rust APIs.

## Decision

**Parity means observable results.** For every corpus model, dive must match the
pinned TLC on these points:

- the verdict;
- distinct and generated counts, at 1 worker;
- the violated property name;
- the error class and exit code;
- the depth;
- a counterexample that the independent validator accepts.

dive does not emulate arbitrary or nondeterministic TLC internals, such as
wall-clock timing, thread or queue service order, tableau numbering, SCC or lasso
choice, Java method projections or admission proofs. ADR 0010 still holds: dive
does better by default where approved, and `--compat=tlc` restores TLC's
observable behaviour.

**Evidence policy.** Run outputs, logs, stdout and stderr dumps, `.gz` files,
source snapshots and `before-*` directories are never committed. Committed
evidence is compact JSON or Markdown of at most 100 KB per item. Bulky evidence
is regenerated from pinned inputs. Test fixtures hold only the inputs and expected
outputs that a test actually reads. Goldens under `conformance/golden/` stay
harness-generated. `scripts/check-repo-budget.py` enforces a file-size, file-count
and snapshot-pattern budget from the pre-commit hook.

**Lean CI, full gate locally.** Required CI runs a fast workspace gate and the
conformance gate. The complete local gate is opt-in and is run before pushing
changes that need it.

**Salvage rule.** Functional features are kept and simplified: checkpoint and
recovery, DFID, frontier, predecessor and seen-set spill, simulation and PlusCal
checksums. Emulation of nondeterministic TLC timing or order, and
admission-proof scaffolding, are removed. Tests that only check the
self-consistency of evidence (hashes, manifests, receipts, source identity, git
proofs, epoch indexes) are deleted with that evidence.

**Process.** Changes are small single-scope PRs from `main`. There are no
long-lived integration or compose branches. Each real decision gets one ADR.
Feature notes go in one guide per area, not one doc per increment.

## Consequences

- The tracked tree shrinks from about 660,000 files to about 12,000.
- Some product code that exists only to project TLC internals is now unowned. A
  later stage removes it, together with its tests.
- Scoreboard numbers in `docs/scoreboard.md` replace the per-family parity ledger.

This reset superseded:

- ADRs 0021–0081. Their still-relevant decisions are consolidated in 0082
  (liveness), 0083 (storage, checkpoint and DFID), 0084 (PlusCal) and 0085
  (symmetry). These were emulation-only and are dropped without a successor:
  - 0025, 0027, 0029 and 0048: projections of TLC's planner, tableau, phase L and
    partial SCC;
  - 0050, 0051, 0052, 0054, 0057, 0059, 0062, 0065, 0070, 0074, 0076, 0078 and
    0080: periodic controller, request, terminal and finalization scaffolding;
  - 0064, 0067, 0071 and 0081: tuple and history admission proofs for trace
    replay.
- The parity ledger (`docs/tlc-parity.md` and `conformance/parity-features.json`)
  and the delivery plans (`docs/design/tlc-parity-plan.md`, `m5-plan.md`,
  `m6-plan.md` and `final-temporal-study.md`). `conformance/parity-surfaces.json`
  is kept as a reference inventory.
- `docs/sprint-status.md`, `docs/reviews/` and the per-increment docs, which are
  merged into area guides such as `docs/pluscal-translation.md` and
  `docs/user/storage-recovery.md`.
- `conformance/probes/`, the `crates/dive-pluscal/*acceptance/` trees, the bulky
  parts of `conformance/studies/`, and the evidence checker scripts.
