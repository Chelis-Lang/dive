# Simulation oracle probes: random operators

`oracle.json` holds compact receipts of the pinned TLC (`tlc-2026.10.06.014338`)
in `-simulate` mode, captured by `capture.py`. Raw stdout is not committed;
each case keeps its configuration text, flags, exit, deciding message codes,
generated count, the stdout SHA-256 and the states of each `file=` trace. The
design that uses them is `docs/developer/simulation-design.md` §8; the
decision is ADR 0162 and the divergence D-044.

| Module | Shows |
|---|---|
| `RndNext` | `RandomElement` in `Next` draws a fresh value at every step of every trace (N1, N2) |
| `RndInit` | `RandomElement` in `Init` is drawn once per run: every trace starts in the same state (I1); dive draws per trace (D-044) |
| `RndExists` | one draw per `\E` binding (E1) |
| `RndDeadlock` | a step whose draw disables `Next` is reported as a deadlock, 2114, exit 11 (D1), though another value continues; without deadlock checking the trace ends there (D2); dive draws again and never reports a deadlock |
| `RndSubset`, `RndSubsetNegative` | `RandomSubset` in `Init` and `Next` (S1); a count above the set's size gives the whole set (S2); a count of 0 gives `{}` (S3); a negative count is error 2154, exit 255 (S4) |
| `RndSetOfSubsets` | `RandomSetOfSubsets` in `Next` (R1) |
| `RndInvariant` | a draw in an invariant decides a violation, 2110, on one draw (V1); dive refuses it by name |
| `RndEnabled` | a draw inside `ENABLED` is reported as a deadlock, 2114 (B1); dive refuses it by name |
| `RndConst` | a constant definition draws once at start-up, also with a `VIEW` (C1, C2) |
| `RndConfigured` | a constant configured with `C <- Pick` names `Pick`'s body, which draws again at every use (C3) |

The consumers are `crates/dive-api/tests/simulate_random_ops.rs` and
`crates/dive-trace/src/draws.rs`. Regenerate with the pinned jar and JRE:

```sh
python3 -I capture.py --java "$JAVA" --jar "$JAR" --reason "..."
```
