# Core plan

[ADR 0153](../../decisions/0153-core-is-the-product.md) sets the direction.
[RELEASE.md](../../RELEASE.md) defines the ordered critical path and gates.
Issue #498 is the gate checklist; #784 receives the measured R1–R3 status.
The [scoreboard](../../scoreboard.md) is the current measurement source.

## Stages

| Stage | Outcome | Acceptance |
|---|---|---|
| C0 | Core prototype | Completed; historical results are in [m0-results.md](m0-results.md) |
| C1 | Basic checking measured without fallback | Core-alone full-tier measurement and refusal-reason histogram (#785) |
| C2 | Useful coverage and speed | Canonical default-profile sets (#786), largest refusal buckets first, liveness cost/capacity (#787–#788), bundled libraries (#789), nominal compat (#790); R1–R4 |
| C3 | Core becomes the only checking runtime in the product path | R1 holds; remove Auto fallback and Reference checking runtime, keep shared startup/validator dependencies, replace shadow comparison with goldens (#791) |
| C4 | Stable surface and release | CLI/report goldens (#792–#793), packaging and one RC qualification (#794); R5–R6 |

Modes, storage/recovery breadth, distribution, DAP, IOUtils/IOEnv, TLC coverage,
LSP and speculative performance caches are frozen until v0.1. Existing code
stays merged; it does not create a prerequisite for this release.

## Working method

Assign independent issues across at most three live worktrees, each with its
own writable build directory. Integrate small PRs through required CI and
prune completed worktrees. Changes use focused semantic and regression tests;
full qualification is reserved for release or engine cutover.

Canonical default-profile sets replace the normalization-history, receiver
and authority workstream. Compat-only fidelity does not justify significant
engineering. Keep decisions in ADRs, current numbers in the scoreboard and
remaining work in the existing release issues. Historical stages and
measurements remain in git.
