# Divergences from TLC

`dive` matches the pinned TLC by default (charter, soundness contract rule 5). Each entry below is an **opt-in** behaviour that is stricter or more correct than TLC. Every entry is named, has a rationale and an oracle note, and is tested in both modes.

| Name | Default (TLC-compatible) | Opt-in behaviour | Rationale | Oracle note | Tests |
|---|---|---|---|---|---|

No divergences are implemented yet.

## Known TLC behaviours that are candidates

These are recorded so that nobody resolves them by quiet choice. Each needs a minimal spec and an oracle result before implementation.

- **Integer overflow.** TLC integers are 32-bit, and overflow raises `EC.TLC_MODULE_OVERFLOW` (2178) rather than wrapping (`tlc2/module/Naturals.java`, `Integers.java`). Literals above 2^31−1 are rejected (`SpecProcessor.java`). Default: match the error. Candidate opt-in: arbitrary-precision integers.
- **Symmetry with liveness.** TLC warns that symmetry during liveness checking "might cause TLC to miss violations" (`EC.TLC_FEATURE_UNSUPPORTED_LIVENESS_SYMMETRY`). Candidate opt-in: refuse the combination.
- **Counterexample choice and depth with multiple workers.** TLC is only approximately breadth-first with more than one worker (`TLCTrace.java`), so trace length and reported depth can vary (tlaplus#883). `dive` is deterministic at every worker count. See ADR 0004.
- **`CHOOSE` over unordered sets.** TLC picks according to its value ordering. To be characterised.
- **Output ordering.** To be characterised.
