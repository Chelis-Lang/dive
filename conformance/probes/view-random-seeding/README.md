# VIEW and random expansion: native admission boundary

This is a preserved native defect (#178), not a proposal to change TLA+ or TLC.
The old expansion path seeded from the raw predecessor, while pinned TLC's
`RandomEnumerableValues` asks `TLCStateMutExt.fingerPrint`, which applies the
configured VIEW (after any symmetry representative).

The [minimal module](before/RandomView.tla) has `V == n`, a random successor `x`,
and invariant `n < 3`. With seed 7, fingerprint polynomial 0 and one worker,
clean TLC and the old native checker both exited 12 with four generated/distinct
states and depth four. Their concrete `x` traces differed:

| Producer | Concrete x values |
| --- | --- |
| Clean pinned TLC | 0, 78, 49, 21 |
| Old native expansion | 0, 80, 94, 100 |

The [original index](before/index.json) pins the exact old native source commit,
executable SHA, oracle SHA, commands, source and raw output hashes. The original
bytes are unchanged. The compressed old engine and pinned
[TLC state fingerprint source](before/tlc-state-source/index.json) explain the
seeding distinction. Separate [validation controls](before/validation-index.json)
retain both concrete traces and their existing independent `Unsupported` result;
no false independent certificate was demonstrated.

Current ordinary safety checking conservatively refuses any configured VIEW
when the lowered source refers to a random native operator. Refusal precedes
binding, ASSUME, Init, output and search; actual workers are zero and statistics,
collision report and counterexample are absent. This includes unused random
definitions, aliases and typed native overrides. It is intentionally broader
than proven reachability: importing Randomization can include unused source
definitions that call random natives. The preserved initial positive-selection
failure in `development/pure-source-selection-before/` records that case; the
true pure positive imports only Naturals/TLC, with its assertions unchanged. Both profiles, both stores and requested 1/4/8 workers
with chaos are covered by `crates/dive-check/tests/view_random.rs`. Pure VIEW and
random checking without VIEW still run. SYMMETRY/random remains refused by its
existing independent limitation.

```sh
dive check conformance/probes/view-random-seeding/before/RandomView.tla --compat=tlc --seed 7 --fp 0 --workers 8 --output json
```

This now returns `unsupported` with the named transformed-fingerprint seeding
and independent-replay requirement. It does not claim the model is safe. Full
support requires matched concrete draws and independent replay, not just removal
of this guard or comparison of counts. Historical source authorities remain
archived separately from current-source integrity checks.
