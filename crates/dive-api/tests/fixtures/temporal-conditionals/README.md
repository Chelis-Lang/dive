# Temporal conditional oracle probes

Generated with the pinned 1.8.x oracle on 2026-10-09, using `dev capture`
with the commands recorded alongside the raw outputs. Reason: validate pure
pointwise temporal IF and ordered CASE with OTHER, including state/action
and temporal guards. The deciding run has no dumpTrace flag; any trace is
from the separate cross-checked pass. Profiles and stores are checked against
truth and counts, not TLC-exact lasso selection or finite-prefix stopping.

`CdState`, `CdFair` and `CdTemporalGuard` produce temporal violations or
success with 5 generated / 2 distinct / depth 2. `CdAction` instead exits
255 with 2214 (`TLC_LIVE_WRONG_FORMULA_FORMAT`); `CdCaseOverlap` exits 255
with 2213 (`TLC_LIVE_CANNOT_HANDLE_FORMULA`). They are preserved as negative
controls: both dive profiles refuse these constructors by name rather than
claiming conformance. Broader conditional support is a recorded candidate,
not an adopted result-changing default.
