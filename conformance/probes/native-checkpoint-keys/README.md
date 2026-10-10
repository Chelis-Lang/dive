# Configured native checkpoint keys

The clean pinned primary captures precede the implementation. At workers 1,
seed 7 and fp 0, CpView reports 7 generated / 4 distinct / depth 4; CpSymView
reports 5 / 3 / 3; CpSymConcrete reports 3 / 3 / 3. Each exits 12 with a concrete
invariant counterexample. The frozen ordinary native checker agrees while its
checkpoint entry point refuses VIEW/SYMMETRY explicitly.

CpSymConcrete starts with owner p2, whose canonical symmetry representative is
p1. Concrete transitions require owner to remain p2. Swapping a witness to the
equivalent representative must not become a legal Init or Next proof. This is
an intentionally asymmetric retention control, not symmetry-audit evidence.

`primary/index.json` preserves source, jar, baseline binary and raw transcript
hashes. Reproduce using the frozen baseline binary and fresh output:

```sh
python3 -I conformance/probes/native-checkpoint-keys/capture_primary.py JAVA JAR BASELINE_DIVE OUT
```

The extension is specified in ADR 0038. Historical checkpoint controls and
oracle goldens remain unchanged. Reduced keys certify deduplication only;
archives remain trusted-local inputs, and witnesses are checked independently.


Native recovery controls run both profiles and stores, widths requested at 1/4/8,
chaos off/19 and completed boundaries 0/1. All native executions report inline
width one. The 144 groups include TLC counter reset, explicit Retain equality,
concrete p2 witnesses, independent positives and equivalent-key Init/Next/last-
state negatives. The three API tests split the same full matrix by source case
so no single CI test needs the entire matrix's runtime. Key mismatch and failed
configured-VIEW evaluation with coherent checksums refuse as archive admission
failures. Neither archive integrity nor symmetry declaration validity certifies
spec symmetry or reachability.

```sh
python3 -I conformance/probes/native-checkpoint-keys/capture_recovery.py DIVE FRESH_OUT
```

The initial stricter-refusal failures and exact corresponding source/tests are
preserved under `development/`: unused TLC documentation bodies and unused
native instance imports cannot be executed simply because they appear in the IR.
An initial shadow control was invalid TLA+; its exact rejected source remains.
The corrected named-instance controls keep the original positive/refusal
assertions and verify actual `Def.std` override authority. User same-name bodies,
config replacements and effectful VIEW/symmetry paths cannot evade admission.
The original changed parent checkpoint source is kept separately. Historical
first-checkpoint and public-context source matrices remain untouched.


The final release study passed all 144 groups: 576 full/pause/actual-resume checks
and 432 independent witness validations, totaling 1,008 preserved CLI runs.
`native-alias-fixed/index.json` pins the final executable and source separately from the
primary baseline. Every raw output, validation input and archive hash was checked.
The final workspace gate passed 738/738 tests and all 20 Python controls,
doctests/rustdoc, layering, standalone trace and dependency checks. CM1's 21
models and all 16 ValidationTestSuite smoke models fully agreed at 1/4/8 workers.
`delivery.json` identifies the final matrix and log hashes. No oracle goldens or
historical evidence were regenerated or edited.

The earlier successful 1,008-run study in `native/index.json` and 736-test gate
remain historical pre-startup-guard evidence. Their exact changed source bytes
and manifest are preserved under `development/before-standard-startup-guard/`.
They are not labelled as final-source results. An interrupted subsequent gate is
preserved with its frozen source/test bytes under
`development/interrupted-standard-local-root-gate/`.

A final strict admission control exposed an unused standard-classified nonnative
top-level zero-arity body executing PrintT during binding. Standard modules participate in
that startup walk too. The before-repair source, green historical gate, complete
historical native matrix and failing effect control remain separate evidence.
The final guard roots those top-level startup bodies as well; standard local
zero-arity closure definitions are not independent startup roots (the reachable
SelectSeq positive control retains its captured operator argument). It never uses standard classification to hide a real body or changes
the native override authority.


The separately preserved `native-final/index.json` study and 737-test gate are
historical evidence before the context-alias admission repair. Their source,
manifest, raw failing regression and read-only root reproductions live under
`development/before-context-alias-guard/`. Issue #164 demonstrated that a valid
Rust module context could select an effectful local constant replacement without
its definition name matching the configured alias. Before the repair, binding
printed TRUE and committed an incomplete archive. The final guard resolves role
and substitution roots through matching module-context definition IDs before
binding. The paired pure-alias positive still pauses successfully; the effectful
alias produces no output or archive. This is a dive admission defect, not a TLC
behavior difference. Direct native-node, invalid-child-order and invalid-scope
scratch controls are retained separately rather than counted as successes.

Final `*-alias-fixed.log` files correspond only to the current 738-test source
and 1,008-run matrix. Older `*-final.log` and unsuffixed logs are historical.


## Current-source integration

The 738-test delivery and its `native-alias-fixed` matrix above are immutable
historical feature evidence from commit `099e7a45`, not evidence for later runtime
source. Its complete source authority is archived under `historical-delivery-source/`.
This separate integration starts from `4715fb54`, retaining the current frontier
Queue/predecessor request fields, pre-effect IR/context admission, the six-way
baseline API matrix split and direct public-IR alias refusal control. The exact
parent checkpoint and combined-source authorities are preserved under
`integration-parent-source/` before current authorities are regenerated. Current
release captures and gate records will be labelled separately; no historical
raw bytes, source claims or goldens are replaced.
