# Closed pure VIEW RandomElement streams

This ordinary safety slice selects the stream from the VIEW fingerprint, while
retaining each concrete predecessor for trace replay. It keeps the original
wrong-answer/refusal evidence in `../view-random-seeding/`, and the diagnostic
prototype's complete 84 clean/native rows in `before-current-integration/`.
Those rows retain their own source/binary identities and are not a shipping gate.

At the pinned `94d0c50`, `RandomEnumerableValues.TLCStateRandom.initializedFor`
asks the actual state's `fingerPrint()`. `TLCStateMut.fingerPrint` normalizes
concrete variables, applies any symmetry representative, then evaluates VIEW.
This slice does not admit random symmetry. Strict streams use the resulting
VIEW FP64 XOR seed; default streams keep the approved replayable D-002 algorithm.

```sh
dive check conformance/probes/view-random-stream/primary/scalar/configured/ViewStream.tla --compat=tlc --seed 7 --fp 0 --workers 8 --output json
```

The Rust API consumer runs the same check and independently validates the
emitted concrete document:

```sh
cargo run --locked -p dive-api --example view_random
```

See `crates/dive-api/examples/view_random.rs` for the complete request and
`SpecModel`/`TraceDoc` replay flow. A checker verdict alone is not a certificate.

The scalar projection is `n`; the configured call is `C(1..100)` with
`CONSTANT C <- RandomElement`. Strict output has generated/distinct/depth 4/4/4
and concrete x values 0,78,49,21. Requested width 8 is reported separately from
actual inline width 1. Direct calls use the same transformed authority. Counts
alone would not have caught the old wrong concrete values 0,80,94,100.

Only closed pure VIEW and startup/Init contexts are admitted. Opaque parameters,
substitutions, effects, runtime-name constructors, Randomization families and
unproved configured authority refuse before binding. The source-wide scan can
refuse unused bodies or unused imports; it does not claim precise reachability.
Both concrete and projected values have a 16,384-visit/depth 64 admission bound:
atoms, literal tuple/record children, general functions with homogeneous atomic
domains, and stored homogeneous atomic sets. Deferred/symbolic sets, compound or
mixed sets and nonliteral tuple/record shapes stay refused. This check invokes
no provider/comparison/normalization callback and allocates no input-sized work
vector. Successful shape admission is not itself a semantic certificate.

The first-successor random counterexample boundary remains `unsupported` with
its stopping prefix and no witness. RNG startup effects, symmetry, other native
random families, graph/temporal checking and seeded recovery are not lifted.
The #190 mixed-set JSON decoder/default-normalization gap remains open; this
increment gives those shapes no new certificate credit.

`capture_primary.py` retains all seven original shapes (six projections plus
no-VIEW), direct/configured routes, polynomials 0/7 and seeds 0/7/14/42/123456789:
140 clean single-worker primary observations without dumpTrace. Original 84
observations are preserved separately, not replaced or retagged. Current native,
mutant and independent replay controls must be tied to their exact producer.

The exact current producer and gate are recorded in `current/receipt.json`.
Its full workspace gate passed 1,144 tests with zero skipped. Fresh runtime
controls cover all 140 new oracle rows at both profiles/stores, widths 1/4/8
and chaos scheduling: 3,360 runs, 240 independently valid emitted words and
720 rejected mutants. Store/fingerprint/collision metadata is asserted for each
store before the specifically named equality allowlist is applied. The original
consumer's mistaken indexing of exact-store nullable fingerprint metadata is
retained; correcting that consumer did not change runtime Rust or the oracle.

Ten additional clean first-successor captures print only the initial state.
Their 240 native runs remain explicitly unsupported. Twenty separately supplied
full words validate, and 60 altered words fail; that is not agreement with TLC's
partial printed trace. Another 192 source-wide admission controls refuse before
output/search, including unused impure bodies and an unused random-family import.
Fresh CM1's 21 models and ValidationTestSuite's 16 smoke models pass at 1/4/8.
The executable Rust API guide also passes. Current receipts retain their own
source/binary identities; the original 84-case prototype remains separate.

Run `python3 -I scripts/check-view-random-stream.py` to verify the immutable
source/evidence and exact selection accounting. Invalid ending-name mutants are
rejections, not additional semantic Next-certificate credit. #190's mixed-set
exclusions and all other named restrictions above remain open.
