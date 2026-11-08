# Runtime name ordering regression (#122)

`DynamicStringOrder` creates strings during successor evaluation and selects
between them with `CHOOSE`. A speculative pool can intern them out of TLC's
single-worker encounter order, including a pool of one under chaos scheduling.
`DynamicStringSymmetry` keeps the same regression on the safety symmetry path.

Clean primary captures are under `tlc-2026.10.06.014338/`. They use the pinned
jar (SHA-256 `7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d`),
Temurin 17.0.20.1, `-tool -workers 1 -seed SEED -fp 0 -cleanup`, and a separate
metadata directory. There is no `-dumpTrace` pass. JSON sidecars record the
source/config hashes, flags and exit status; stdout/stderr are unmodified.

TLC reports exit 12, depth 2, 15 queued states, with generated/distinct counts
30/30 at seeds 0 and 14, and 23/23 at seed 7. At seed 7, the violating transition
is `x = 6, s = "unset"` to `x = 6, s = "7"`. TLC eagerly interns the seed string
when it creates the checker; omitting this startup step changes the result.

The tests compare both stores and 1/4/8 requested workers, with and without
chaos. TLC order forces inline evaluation and reports one actual worker.
Canonical default order is separately checked and retains parallel admission.
Wrappers and higher-order operator arguments must preserve the restriction.
