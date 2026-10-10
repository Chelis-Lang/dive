# Bounded final-mode safety-only route

This increment removes a vacuous-obligation routing refusal, not a temporal
planning/product/SCC limitation. A freshly bound model with no implied temporal
properties uses the actual ordinary safety engine, bounded by the unchanged
capture ceilings. Private receipts retain actual completion/stopping verdicts,
counts, errors and worker facts. Finite selected counterexamples are replayed
against a fresh model in memory and again as lossless JSON. A result flag or
empty property list does not supply route or validation authority.

`primary.json` records the complete nine-model clean pinned selection, exact
source/config/raw hashes, flags and reason. `raw` preserves the real clean TLC
stream/exit and separately cross-checked trace pass, plus SANY runs. No golden
was edited or manually created. The first `SafetyDeadlock` source used constant
`Next == FALSE`: TLC rejected it at binding (`TLC_CONFIG_ID_MUST_NOT_BE_CONSTANT`).
That complete observation/source is preserved under `history-constant-next`;
the current action-level `x'=x /\ FALSE` really deadlocks with code2114/exit11.
Initial violations are inline initial-state messages and carry no TLC search
summary; numbered trace length zero does not mean no violating initial state.

Regenerate raw captures using a release harness and private verified cache:

```sh
dive-conformance dev capture --oracle tlc-2026.10.06.014338 --specs conformance/probes/tlc-final-safety/models --out conformance/probes/tlc-final-safety/raw --seed 0 --fp 0 --lncheck final --workers 1 --reason "Review bounded final safety route" SafetyHold SafetyInvariant SafetyInitial SafetyAction SafetyPropertyInitial SafetyDeadlock SafetyIf SafetyConstrainedAction SafetyConstrainedInitial
```

`SafetyInfinite` has no unbounded primary run. Its actual native states=3 control
must retain ResourceLimit/incomplete evidence. At most three graph
representatives are retained; engine insertion precedes collector admission, so
the deciding fourth inserted state appears in stopping counters. This is not a
three-state aggregate memory/seen-set/byte budget and can never issue success.

Focused controls compare real deciding TLC classes and all reported counts at
1/3/4/8 widths, prove independent rejection of wrong-Init/length mutations, and
separately derive routing from fresh temporal-versus-safety source. CLI controls
preserve ordinary schema1 and explicit schema2 route facts. Harness controls
reject forged obligations/product certificates, false completion, count/ceiling
contradictions and pending frontier claims. It also freshly binds claimed safety
sources before comparison credit; a real temporal source contradicting the claim
becomes unclassified rather than an excused agreement.

Implied initial properties plus state constraints, and implied action properties
plus state/action constraints, remain pre-pool named refusals until independent
replay proves TLC's earlier checks on out-of-model states. Existing effect,
opaque capture, runtime-name, VIEW/symmetry/randomness and storage refusals remain.
The old complete 140-Unsupported studies remain untouched. This increment earns
only supported safety-route evidence, not general temporal/periodic/family parity.

See [the user API/CLI guide](../../../docs/tlc-final-api.md#safety-only-route).

## Before limits retention

`history-before-limits-receipt/` preserves the complete successful 865-test
workspace gate, its 584 build-related input hashes (including archived helper
Rust sources), 871 source/evidence hashes and a byte-exact source archive. That
producer preceded actual caller-limits retention; none of its observations is
retagged as a limits-follow-up run. The current receipt carries exact caller
`Limits`, and new API/JSON controls exercise non-default ceilings separately.
The retained-representative ceiling still permits the actual deciding fourth
inserted state in the incomplete three-representative stop; this is not an
aggregate memory budget or successful completion.
