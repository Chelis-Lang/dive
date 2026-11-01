# 0139: Native finite modes with isolated engine selection

- **Status:** accepted
- **Date:** 2026-11-01

## Context

The compiled core already supplies the plan, value machine, Init/Next generator
and predicate evaluator used by native BFS. Finite simulation and DFID still
run on the reference evaluator. Their public reports and independent source
replay already have distinct sampling/completion contracts (ADRs 0083 and 0116).
A whole-run retry after a reached refusal can duplicate effects. Even a pure
lazy argument evaluated by compiler handoff can change TLC normalization/cache
history, so speculative preparation must not touch the fallback runtime.

## Decision

- Keep existing file APIs and command defaults on `reference`. Add
  `simulate_file_engine` and `dfid_file_engine`, and extend the existing CLI
  `--engine reference|core|auto` / `DIVE_ENGINE` selection to both finite modes.
  Explicit `core` never falls back. Finite-mode `auto` selects a whole reference
  run only before execution and reports the static reason; reached native
  refusals never restart a mode.
- Probe feasibility using the existing immutable source/configuration Recipe
  with fresh names, evaluator caches and controlled binding, without rereading
  files or executing assumptions/steps. Discard that probe's runtime state.
  After selecting the actual native backend, evaluate its assumptions once,
  then compile the production plan against that actual startup state. This
  preserves assumption-induced demand/history and leaves fallback state intact.
- Execute finite modes through the existing Plan/Machine, preserving their
  generator frames, predicate phases and owned construction demands. Private
  file-mode drivers live beside the existing API report adapters, so they can
  reuse simulation streams, action flows, reports and independent certification
  without adding crate edges or a second interpreter. BFS is unchanged.
- Reuse the simulation host/lease for evaluator-backed assumptions and output.
  Native Print preserves argument/rendering order and shares the remaining
  event/byte allowance. Unsupported native effects refuse when reached;
  declared unavailable effects can select the reference backend before Auto
  execution. Both backends currently use one actual worker.
- Keep DFID's visitation status, iteration accounting and first deciding-event
  drain order. Exact keys and fingerprint collision guards compare closed
  mathematical rows with Heap equality, including the no-hashcons ablation.
  Fingerprint bits remain private engine keys (ADR 0117). Infinite/unclosed
  unsupported states cannot gain a completion certificate.
- Keep each Init/action candidate batch live until its real constraints, key
  normalization and property demands. Retain only required DFS rows with their
  actual views, then roll back the batch arena after frames have closed. Do
  defer generator-internal continuation reclamation while collecting an eager
  DFID vector: an emitted row may still refer to those continuation transients.
  Restore normal reclamation before deferred checking and reclaim the complete
  batch afterwards, including across repeated iterative levels. Do
  not snapshot a raw local receiver before its demand or widen the existing
  construction-history admission. Persistent stores, seen keys, active DFS
  frames and retained reports remain separate memory consumers.
- Use one existing run-budget owner through producer work and fresh independent
  finite-word replay. Typed cancellation is separate from actual preflight
  resource refusals; only measured exhaustion produces `stopped` / exit 5.
  Simulation never proves safety. DFID proves only with its existing visitation
  completion certificate. Every reported violation still needs independent
  validation of the actual word.

## Consequences

Native finite execution adds useful core modes without changing the default
engine, report schema 3 or native trace versions 1–6. Reports add actual and
requested engine names and any static fallback reason. TLC draw equality is
not a simulation acceptance condition (ADR 0116); replay includes the engine,
seed and options. Initial feasibility and production compilation are separate
startup costs. New mode-wide throughput, native checkpoint/recovery, disk seen,
multi-worker simulation and broader effects remain separate work.
