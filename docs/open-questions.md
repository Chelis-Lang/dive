# Open questions for the owner

Each question is open until it is answered here or recorded in an ADR.

1. **License.** Which licence should dive use: MIT (the older org repos), `Apache-2.0 OR MIT` (barnacle), or something else? It must be decided before any public release (ADR 0007).
2. **Integer semantics opt-in.** Should arbitrary-precision integers be an early named divergence, or deferred until after M5? TLC raises an overflow error at 32 bits; see `docs/divergences.md`.
3. **Liveness plus symmetry.** Should `dive` refuse liveness checking under symmetry by default, as a divergence from TLC, which only warns? The charter defaults to matching TLC, so this needs an explicit owner call.
4. **Oracle cadence.** How often should the re-hosted TLC 1.8.x snapshot be refreshed? Proposed: quarterly, or when the Examples corpus pin moves, with a recorded golden diff.
5. **Role of v1.7.4.**
   - **The problem.** v1.7.4 branched from upstream master on 2020-04-25, and no CommunityModules release tried runs with it. It therefore runs without CommunityModules, so every Examples spec that uses them disagrees on that oracle (ADR 0003).
   - **The question.** Should v1.7.4 stay a cross-check for core semantics only? Or should a second, older 1.8.x snapshot replace it as the drift oracle?

   Either way the decision is recorded.
6. **Module library pins.** The TLAPS library (tlapm `library/`) and Apalache's modules are now pinned by commit, as upstream Examples CI uses them unpinned. Should they follow the same refresh cadence as the oracle?
