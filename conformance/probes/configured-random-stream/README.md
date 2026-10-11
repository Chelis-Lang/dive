# Actual-bound RandomElement streams

The previous admission epoch is frozen in `../configured-random-admission/`.
This continuation implements the ordinary untransformed RandomElement route;
configured Randomization families, VIEW/SYMMETRY randomness, graph/temporal
capture and seeded recovery remain explicit refusals. Family parity remains
partial. See `docs/user/checking.md` for the runnable checking contract.

```tla
---- MODULE Stream ----
EXTENDS Naturals,TLC
CONSTANT C(_)
VARIABLES n,x
Init == n=0 /\ x=0
Next == n<3 /\ n'=n+1 /\ x'=C(1..100)
Inv == n<3
====
```

```text
CONSTANT C <- RandomElement
INIT Init
NEXT Next
INVARIANT Inv
CHECK_DEADLOCK FALSE
```

```sh
dive check Stream.tla --compat=tlc --seed 7 --fp 0 --workers 8 --output json
```

The admitted run reports actual inline width 1. Clean pinned TLC at seed7/fp0
prints the concrete values `0,80,94,100`, generated/distinct/depth `4/4/4`.
The actual bound native target participates in execution planning and stream
selection even when no source node names RandomElement. The standalone validator
resolves the same authority without enabling successor enumeration.

## The replay boundary is separate

With `Inv == x # 80`, clean TLC reports exit12, generated/distinct/depth `2/2/2`,
but prints only the initial `x=0` state. The direct source-call counterpart does
exactly the same thing. `TLCTrace.printTrace` reuses the initial predecessor
object in its fast path; `Tool.getState(s2,s1)` invokes RandomElement again, and
`TLCStateRandom.isInitializedFor` retains the already-consumed stream because
the object identity has not changed. The deeper `Inv == n<3` case reconstructs
fresh state objects and prints the complete trace.

Both dive profiles currently return named Unsupported at the first-successor
random trace boundary, retain observed stopping-prefix statistics/actual worker
use, and attach no counterexample certificate. A full trace which happens to
validate independently does not prove it matches TLC's printing. No new default
full-witness override was adopted. Draws before random Init likewise remain
outside a fresh-Init independent certificate unless their exact prefix is known.

`run_primary.py` captures clean single-worker seeds0/7/14 for 18 startup, Init,
branch, call and binding variants. The direct-source counterpart is captured
separately; raw outputs are retained before any native comparison. It never uses
`-dumpTrace`. `run_native.py` compares concrete states as well as verdict/counts,
preserves explicit replay refusals, checks repeated requested widths1/4/8 with
chaos, and runs valid/changed-Init standalone validation controls. Test assertions
compare all API fields except precisely named elapsed/source-location differences;
source spelling changes action columns, not concrete semantics.

`before/` retains the baseline wrong-answer test, previous guarded tests, all
failed comparison/compilation observations and the raw authority/parser history.
Current runtime producer, source inputs, receipts and complete selections are
recorded separately; historical guard receipts are never retagged as supported
stream evidence. SHA checks prove captured bytes/integrity, not an independent
archive authenticity or reachability certificate.
