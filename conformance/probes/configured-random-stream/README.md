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
The detector is source-wide and may conservatively refuse an initial-successor
violation because of an unused random definition. This is not a claim that every
first-successor TLC trace is incomplete.

The original `f9262d19900f58fbeb875e4651b6bdd944e1e35d` full gate is retained
under `before/f926-failed-gate/`: 1049/1054 passed, five failed, zero skipped.
Its standalone campaign had 132 valid originals, 48 unsupported originals and
180 changed-Init controls rejected. The 48 originals comprise 12 each for
two draws, a discarded random branch, random Init and conditional random Init.
The latter two refusals exposed a startup-root regression: the nested zero-arity
`Sequences.SelectSeq` LET `F` captures `Test(_)` and is not evaluated independently
at startup. The repaired proof inspects actual top-level startup roots; all
existing random Init, seed/polynomial and redraw controls remain strict.
Those original observations remain historical and are not retagged as the
corrected producer's results. Missing traces and standalone Unsupported originals
never constitute full conformance agreement or an independently validated witness.

The corrected runtime producer is
`2c2ebda3940109fae34a033749b63a8560f8834c`; its metadata-only gate source is
`4cbe698ca97eb0469ebcaaa0a9700ffb89cb0dee`. All 434 compiled Rust/Cargo inputs
were checked for equality, without retagging either execution. The corrected
full gate passes 1,054 tests with zero skipped and all eight stages passed.
Its fresh campaign retains 1,296 checker observations, 648 clean-reference
printed-output comparisons, and 360 independent validation controls: 144 valid
originals, 36 unsupported originals and 180 invalid changed-Init controls.
The remaining unsupported originals are 12 each for two draws, a discarded
random branch and conditional random Init. Random Init without an unproved
startup prefix now passes its unchanged independent controls.

The complete [current receipt](current-receipt.json) records source archives,
binary hashes, timing, exclusions and all retained files. The CM1 21-model and
ValidationTestSuite 16-model gates fully agree at 1, 4 and 8 requested workers.
These gates do not upgrade every random checker observation to full agreement:
36 first-successor strict controls refuse trace printing, 36 strict startup-Init
controls have no trace, and independent replay can still refuse a supplied word.
The runnable guide control and its raw JSON are under `current/guide/`.

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
