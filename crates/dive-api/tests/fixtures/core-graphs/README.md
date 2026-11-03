# Compiled graph overrides

`GraphsStep` exercises all six Graphs/UndirectedGraphs overrides in an invariant
whose graph depends on the current state. `GraphViolation` changes a graph through
EXCEPT and reports an independently checked violation after two transitions.
Both have generated/distinct/depth 3/3/3 at one worker. The API test consumes
`oracle.json`, compares both profiles and requested widths 1/4/8, and rejects a
forged intermediate state. Folding, memoization and hash-consing controls ensure
the checks do not depend on successful constant evaluation or handle identity.

The observations were generated from pinned TLC `tlc-2026.10.06.014338`
with the pinned CommunityModules jars, one worker, seed 0 and fingerprint 0.
The compact record binds source/configuration and jar hashes. Regenerate by
running each model through that oracle with `-tool -workers 1 -seed 0 -fp 0` and
its adjacent configuration. Raw output belongs in the external evidence archive.
