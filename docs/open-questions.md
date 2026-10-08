# Open questions for the owner

Each question is open until it is answered here or recorded in an ADR.

1. ~~**License.**~~ **Answered 2026-10-08: MIT** (ADR 0007). Original question: Which licence should dive use: MIT (the older org repos), `Apache-2.0 OR MIT` (barnacle), or something else? It must be decided before any public release (ADR 0007).
2. **Integer semantics opt-in.** Should arbitrary-precision integers be an early named divergence, or deferred until after M5? TLC raises an overflow error at 32 bits; see `docs/divergences.md`.
3. **Liveness plus symmetry.** Should `dive` refuse liveness checking under symmetry by default, as a divergence from TLC, which only warns? The charter defaults to matching TLC, so this needs an explicit owner call.
4. ~~**Oracle cadence.**~~ **Answered 2026-10-08: a single snapshot for now, with no cadence** (ADR 0002). Original question: How often should the re-hosted TLC 1.8.x snapshot be refreshed? Proposed: quarterly, or when the Examples corpus pin moves, with a recorded golden diff.
5. ~~**Parallel workers by default.**~~ **Answered 2026-10-08: yes.** Result-preserving enhancements may default to on (ADR 0010).
