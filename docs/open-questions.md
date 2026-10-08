# Open questions for the owner

Each question is open until it is answered here or recorded in an ADR.

1. **License.** Which licence should dive use: MIT (the older org repos), `Apache-2.0 OR MIT` (barnacle), or something else? It must be decided before any public release (ADR 0007).
2. **Integer semantics opt-in.** Should arbitrary-precision integers be an early named divergence, or deferred until after M5? TLC raises an overflow error at 32 bits; see `docs/divergences.md`.
3. **Liveness plus symmetry.** Should `dive` refuse liveness checking under symmetry by default, as a divergence from TLC, which only warns? The charter defaults to matching TLC, so this needs an explicit owner call.
4. **Oracle cadence.** How often should the re-hosted TLC 1.8.x snapshot be refreshed? Proposed: quarterly, or when the Examples corpus pin moves, with a recorded golden diff.
