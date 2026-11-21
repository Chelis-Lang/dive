# Simulation oracle probes: temporal properties, views, symmetry and scale

`oracle.json` holds compact receipts of the pinned TLC (`tlc-2026.10.06.014338`)
in `-simulate` and `-generate` mode, captured by `capture.py`. Raw stdout and
TLC's trace files are not committed; each case keeps its configuration text,
flags, exit, deciding message codes, printed counterexample states, loop
target, stuttering marker, generated count, the stdout SHA-256, and the names
of any `file=` trace modules. The design that uses them is
`docs/developer/simulation-design.md`; the decision is ADR 0162.

Every module is the smallest one that shows its behavior:

| Module | Shows |
|---|---|
| `Cycle` | loops that close (C1 to C6, C10), a depth that never closes the loop (C7, C8), stuttering without fairness (C9), `num` counted per worker (C11) |
| `Terminal` | a trace with no successor is infinite stuttering (T1 to T3); deadlock decides first (T4) |
| `Unbounded` | truncated traces leave liveness silently unchecked (U1 to U3, U9); bad prefixes of `[]P` and `[][A]_v` (U4, U7); constraint cuts (U10, U13, U14); implied initial properties are never checked by `-simulate` (U12, with the BFS control B-U12); ALIAS rendering and `file=` output (A1 to A3) |
| `Fair`, `Strong` | WF and SF decide which lassos are behaviors (F1 to F3, S1, S2) |
| `ViewLoop` | under `VIEW`, TLC closes the loop on the view and reports a counterexample that is not a behavior (V1, BFS control B-V1); VIEW leaves safety unchanged (V3, V4) |
| `SymLoop` | under `SYMMETRY`, TLC misses a real violation (Y2) that it finds without it (Y3); the BFS control B-Y2 reports an unfair stuttering counterexample; SYMMETRY leaves safety unchanged (Y4) |
| `BigNext`, `InfInit`, `InfNext` | `-simulate` enumerates every initial state and every successor (G1); `-generate` builds one successor but still enumerates Init (G2); Init or successors over `Nat` end in GENERAL 1000, exit 255, in both modes (G3 to G6); dive samples them by default and reproduces the error with `--sample-unbounded off` or `--compat=tlc` (D-040) |
| `Walk` | repeated runs with one and four workers and the same seed: the same counterexample each time, a different one at four workers, and generated counts that vary on every run (W1, W4) |

`truth` is the TLA+ answer over all behaviors; `tlc` assesses TLC's answer
from its sample. Regenerate with the pinned jar and JRE:

```sh
python3 -I capture.py --java "$JAVA" --jar "$JAR" --reason "..."
```

`crates/dive-api/tests/simulate_safety_roles.rs` compares the safety cases
(C5, C6, U4, U6, U7, U11 to U14, V3, V4, Y4, A1) under `--compat=tlc`; U12
is compared with its model-checking control B-U12 (D-036). The workers and
temporal phases add tests for the other cases.
