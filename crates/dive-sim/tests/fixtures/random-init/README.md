# Direct scalar Init RandomElement controls

Inputs for the approved partial issue239 simulation increment. Primary capture
comes before production implementation. `oracle.json` is generated only by the
owned primary capture controller after its explicit runtime grant. No expected
random value, trace, count, error code or physical exit is handwritten here.

The consumer is `crates/dive-api/tests/simulate_random_init.rs`. Interval cases distinguish seed
and aril selection from the startup value stream; stored-set/Print cases check
normalization and quiet independent replay; the invariant case checks a real
initial violation; wrong/empty domains retain actual primary errors. The reached
Next-random case intentionally stays outside the supported native subset.

`requested_traces` is regenerated from the actual primary command receipt;
`traces` holds the observed words. Original capture metadata remains preserved
externally. This repairs a count-field collision without rerunning the oracle or
changing observations.

Raw commands/stdout/stderr/physical exits and trace modules stay outside the repo.
This is not a conformance corpus registration and changes no existing population.
