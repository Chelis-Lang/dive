# Open questions for the owner

Each question is open until it is answered here or recorded in an ADR.

1. ~~**License.**~~ **Answered 2026-10-08: MIT** (ADR 0007). Original question: Which licence should dive use: MIT (the older org repos), `Apache-2.0 OR MIT` (barnacle), or something else? It must be decided before any public release (ADR 0007).
2. ~~**Integer semantics opt-in.**~~ **Answered 2026-10-08:** big integers are the intended default, and TLC 32-bit behaviour is built and proven under `--int=tlc` / `--compat=tlc` (ADR 0010). Original question: Should arbitrary-precision integers be an early named divergence, or deferred until after M5? TLC raises an overflow error at 32 bits; see `docs/divergences.md`.
3. ~~**Liveness plus symmetry.**~~ **Answered 2026-10-08:** dive refuses by default, and TLC's warn-and-proceed is built and proven under `--symmetry-liveness=tlc` / `--compat=tlc` (ADR 0010). Original question: Should `dive` refuse liveness checking under symmetry by default, as a divergence from TLC, which only warns? The charter defaults to matching TLC, so this needs an explicit owner call.
4. ~~**Oracle cadence.**~~ **Answered 2026-10-08: a single snapshot for now, with no cadence** (ADR 0002). Original question: How often should the re-hosted TLC 1.8.x snapshot be refreshed? Proposed: quarterly, or when the Examples corpus pin moves, with a recorded golden diff.
5. ~~**Parallel workers by default.**~~ **Answered 2026-10-08: yes.** Result-preserving enhancements may default to on (ADR 0010).
6. ~~**Role of v1.7.4.**~~ **Resolved 2026-10-08:** the 1.8.x line is the oracle, and v1.7.4 is reference-only (ADR 0002 revision). Original question:
   - **The problem.** v1.7.4 branched from upstream master on 2020-04-25, and no CommunityModules release tried runs with it. It therefore runs without CommunityModules, so every Examples spec that uses them disagrees on that oracle (ADR 0003).
   - **The question.** Should v1.7.4 stay a cross-check for core semantics only? Or should a second, older 1.8.x snapshot replace it as the drift oracle?

   Either way the decision is recorded.
7. ~~**Module library pins.**~~ **Resolved 2026-10-08:** they move together with the oracle as one bundle (ADR 0002 revision). Original question: The TLAPS library (tlapm `library/`) and Apalache's modules are now pinned by commit, as upstream Examples CI uses them unpinned. Should they follow the same refresh policy as the oracle, which is currently a single snapshot (ADR 0002)?
