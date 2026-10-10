# Independent review: finite seeded simulation

Reviewed the finite pure single-worker slice against `Simulator.java` and
`SimulationWorker.java` at oracle commit `94d0c50`. This review does not
certify full simulation parity or the whole F-024/F-035 family.

The review found one additional contract mismatch: TLC checks action properties
before constraints, but the exhaustive trace validator requires constraints on
property-ending edges. `SimConstrainedAction` produces TLC code 2112, exit 13;
the original native report called the valid TLC candidate an internal error.
The fixed slice explicitly refuses action properties combined with state/action
constraints before sampling. The exhaustive validator remains unchanged.
Unconstrained action-property violations retain independent certification.

Independent checks passed:

- All ten seed/aril cases: 30 published traces, exact generated counts,
  valid independent concrete traces and no exhaustive-proof claim.
- An external Rust probe over 1,344 combinations of retained budget 0–7,
  trace count 1–4, generated budget 0–6 and depth 0–5: returned documents stay
  bounded, generated counts stay bounded, pending violations are uncertified,
  and sampled outcomes contain the full requested trace count.
- Complete initial enumeration before completeness checks, including an
  arithmetic error following an incomplete first candidate.
- Unreachable random effects refused before backend evaluation, with zero
  generated/initial candidates.
- An invalid sampled transition rejected independently as invalid Next.
- An undecidable deadlock refused, retaining observed code 2114.
- All 104 raw fixture file hashes matched capture provenance.

The final native [control report](../../conformance/reports/simulation-controls.json)
accounts for 23 selected controls: 22 agreements and the constrained-action
named refusal. It records the exact binary hash. Repository regression controls
are in `crates/dive-api/tests/simulate.rs` and `crates/dive-sim/tests/bounds.rs`;
raw pinned transcripts and the reproducible CLI comparison command are described
in `crates/dive-sim/README.md`.

No further blocker was found within this slice. Liveness, runtime effects,
parallel execution, compound initial normalization and alternate schedulers
remain explicit limits, not tested parity claims.
