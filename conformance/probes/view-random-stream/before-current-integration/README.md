# VIEW-backed RandomElement streams

This capture-first continuation retains the existing ordinary VIEW/random refusal
until clean pinned TLC and independent concrete replay establish the admitted
route. The historical wrong-answer repair remains in `../view-random-seeding/`.
D-002 and issue #178 remain partial; symmetry, Randomization families, graph,
temporal and seeded recovery paths remain separate refusals.

At pinned source `94d0c5070852b37b801bb8998c436383ab7ee37b`,
`RandomEnumerableValues.TLCStateRandom.initializedFor` seeds from
`state.fingerPrint() ^ randomSeed`. `TLCStateMut.fingerPrint` first chooses the
symmetry representative, then evaluates the configured VIEW and fingerprints
its value. Thus strict seeding uses the transformed key authority, while a
counterexample must retain and replay its concrete predecessor. Using raw
concrete-state fingerprints for VIEW seeding does not match TLC.

`capture_primary.py` records six VIEW shapes and the no-VIEW control, direct and
configured RandomElement routes, seeds 0/7/14 and fingerprint polynomials 0/7.
Every run uses a clean single-worker primary without dumpTrace. Raw source,
stdout, stderr and exact producer hashes precede implementation changes.
