# Pure state arguments in temporal operators

Captured by the harness against `tlc-2026.10.06.014338`, with the pinned JRE
and module bundle, `-lncheck final -workers 1 -seed 0 -fp 0`:

```sh
DIVE_CACHE_DIR=<verified-cache> cargo run -p dive-conformance --release -- dev capture \
  --oracle tlc-2026.10.06.014338 \
  --specs crates/dive-api/tests/fixtures/state-temporal-calls/specs \
  --out crates/dive-api/tests/fixtures/state-temporal-calls \
  SpHold SpStable SpGuard SpNested SpTuple SpNonstutter SpForwardFair SpPrime
```

Clean transcripts decide counts/truth; violation JSON uses a separate
cross-checked trace pass. `SpHold`, `SpNested`, `SpTuple` and `SpNonstutter`
and `SpForwardFair` succeed; `SpStable` and `SpGuard` violate P. These cover state arguments,
nested forwarding, tuple arguments, a state guard and priming the argument
inside a non-stuttering action, and a fairness action forwarded through the
bound model’s lazy scope. `SpPrime` is a negative control: SANY rejects
bare `<>A` when A is an action without the required `<<A>>_v` form (exit 4;
TLC refuses the module with exit 150).

Final API tests match truth/counts in both profiles/stores and independently
validate every returned lasso. Short-word controls enumerate every boolean
word of length 1–4 and every closing target, checking GF, FG, nested calls,
pointwise IF and recurring non-stuttering changes against direct cycle algebra.
State arguments remain lazy, including through nested forwarding, so they
are evaluated at the actual position/edge. Constant arguments remain values.

Action/temporal and higher-order arguments, hidden checker-register reads and
effects remain planning refusals. Exact TLC selection and phase-L timing are
not claimed. Independent finite ENABLED covers may still refuse wider scopes.
