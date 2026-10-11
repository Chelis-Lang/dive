# Source-owned terminal classification and final composition (#228)

This bounded diagnostic retains every reference selection and failure. It does
not enable ordinary periodic checking or certify a temporal family. The exact
primary jar SHA is
`7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d`.
All selections preserve original source/config, invocation, stdout/stderr,
process exit, timeout and artifact hashes. Actual method returns are separate
from process exits. Transparent observers affect timing; cleanup after method
return is explicitly observer cleanup, never original checker behavior.

## Reference epochs

- `primary-development0`: complete84 rows. The ITool proxy triggers a concrete
  Tool ClassCastException in Worker construction; observed setup failures earn
  no checker credit. Plain CLI invocations remain independently measured.
- `primary-development1`: complete84 rows. Actual FastTool with delegating
  queue/livecheck observers, but an eager post getter incorrectly changes
  observer startup precedence. That observation error remains preserved.
- `primary`: corrected complete84 rows, no eager getter. Includes failed Init,
  source invariant/evaluation errors, empty/nonempty no-action branches, final
  pass/violation, post errors and a finite false-hook run that finishes before
  its hook. A configured hook is not counted as executed without actual events.
- `primary-counterposts`: complete96 rows. TLCGet-based posts discriminate true
  and false results; the native pure guard still refuses them. No native parity
  credit follows from these reference-only cases.
- `primary-postorder`: complete24 minimal rows: four models at1/4/8 with actual
  method and plain CLI observations. These establish the ordered native slice.

```tla
VARIABLE x
Init == x=0
Next == x'=1
Live == <>(x=2)  \* use <>(x=0) for final pass
Post == x=x      \* use FALSE for cached-role getter failure
```

With state Post, the final pass returns0 then post error2405; modelCheck returns
2405 and CLI exits10. A final counterexample calls LiveWorker.printTrace's
counterexample post method: returned2405 is ignored, finalCheck/modelCheck
return2116 and CLI exits13. With cached FALSE, final pass reaches a recorded
2233 role error and modelCheck returns GENERAL1000 (CLI255). A final candidate
instead hits the getter exception during rendering; both actual observer and
CLI exit1 without finalCheck/modelCheck return. The native candidate path
conservatively refuses that render exception and admits no witness. Do not
substitute process exits for absent method returns.

`native-before-8820` retains the unchanged old producer's wrong post-before-final
result: the state Post masked the temporal candidate with2405. Its source,
executable and compile input identities remain separate from this new source.
`development` keeps the adapter and focused-test/clippy failures; corrected
expectations follow exact primary evidence rather than silently rewriting them.
`reference-import.json` hashes byte-exact imported epochs, not current source.

## Private native authority

Actual capture validates complete retained graph rows/statistics before exposing
its EOF callback. Final Phase-L admission verifies original source-id/fingerprint
stamps against each OOS commit, original concrete provenance where retained,
registered identity maps, per-node write coverage, valid roots/transitions and
actual tableau done fingerprints. Reservations use original cumulative graph
quotas before metadata allocation/scans; no aggregate memory claim follows.
Final PEM selection retains OOS/PEM ordering and the first candidate. The normal
postcondition call follows only all successful final selections. The candidate
render call retains returned errors as nondetermining and refuses unproved getter
exceptions; partial request candidates with deferred posts remain named refused.

Each deciding word must be privately reconstructed from original concrete
sources, freshly rebound/reinterred and independently replayed through memory
and JSON. No caller graph, completion boolean or supplied Valid can certify it.
A descriptive FinalSelectionsComplete result is not Holds; all reports keep
model_proof=false. Failed Init, capture/product/SCC limits, malformed ownership,
missing records and source stops cannot become final success.

See [the runnable user guide](../../../docs/terminal-outcome.md) and
[ADR0080](../../../docs/decisions/0080-owned-terminal-finalization.md).
Ordinary admission still requires real asynchronous queue/clock/first-stop/count
composition, Java after-error graph authority, no-action special paths,
checkpoint and complete error/witness contracts. #228 remains open for that
intersection; F-023 remains missing/audit-required. No general corpus gain is
claimed from this slice.


The initial182-case declaration remains in `native-declaration-before-boolean`.
The current selection is declared before any release execution in
`native-runtime-selection.json`: all146 unchanged earlier diagnostic cases plus
60 terminal/postcondition cases at1/3/4/8, total206. The standalone source is
`native-runtime-source.rs`. Its fresh default-library run and full gate will be
recorded separately against a clean immutable producer; declaration is not
execution or matching credit. The new RequestPrefix refusal is exercised at the
existing direct OptionalSnapshot seam with a test-only real returned-output
rendezvous; the public facade's fresh thread intentionally does not inherit that
thread-local seam. Both failed public discriminator assumptions remain historical,
not a public request-path pass. Default release feature isolation remains required.


An additive `primary-boolean-postorder` selection retains36 actual method/CLI
rows at1/4/8 using the exact previously compiled transparent observer. State-level
`TRUE \/ (x=x)` passes after final success; `FALSE /\ (x=x)` yields2404;
`IF TRUE THEN 1 ELSE x` yields2405. With a final candidate each actual method
returns2116/CLI13 despite returned render errors. These prove the positive and
short-circuit grammar path without a synthetic TRUE hook, supplied EOF or
changed default. `boolean-reference-import.json` independently hashes this
closed addition; the original import/epochs remain unchanged.
