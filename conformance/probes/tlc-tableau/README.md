# Pinned TLC particle tableaux and promises (#136)

This diagnostic projection checks actual pinned `makeBinary`, `TBGraph` and
`extractPromises` fields; it does not run or certify strict model checking.
Oracle: `tlc-2026.10.06.014338`, revision `94d0c5070852b37b801bb8998c436383ab7ee37b`.
The SHA-256 and source/selection/raw hashes are recorded in `capture.json`.

`selection.json` retains every selected row: 84 planning inputs from the prior
planning study, 35 already-tagged direct formulas, and 18 FastTool models. The
adapter invokes actual pinned graph/particle methods without evaluating inert
synthetic predicates. It preserves binary syntax, node/particle/state-predicate
order, initial count, edge order (including duplicates), promise order and
fulfillment/empty-accepting metadata. Model controls retain original
configured/Action names separately from derived role labels.

| Outcome | Rows |
| --- | ---: |
| Exact completed field projections | 126 |
| Normalization classifier/cardinality failures | 5 |
| Direct structural/action refusals | 4 |
| SANY-rejected model controls | 2 |
| Total | 137 |

Ten Java processes fail, all retained; no timeouts. The native direct
nonpositive-negation precondition refusal has Java exit 0 and receives no
agreement credit. Empty direct conjunction/disjunction cause Java stack
overflows; native projection returns a typed structural refusal. The original
CONSTANT-substitution INSTANCE control is SANY-invalid; the appended
VARIABLE-substitution control completes. `history-initial-136` preserves all
original sources, the complete prior selection, manifest and raw outcomes.
Prior planning/current/historical captures remain unchanged. Initial development
compiler failure executed no rows and provides no oracle credit; its complete
source/selection/stdout/stderr hashes remain in `history-compiler-failure`.

Native controls in `crates/dive-liveness/tests/tlc_tableau.rs` cover 149,452
periodic formula/tableau comparisons and 5,536 scoped/grouped comparisons to the
independent periodic interpreter, including every closing position. Fresh
whole-witness controls use original configured claims, not derived role labels;
initial/Next/closing edges and forged claims fail independently. The 41
whole-witness checks retain 9 legitimate positives (including two changed
stuttering words) and 32 invalid mutants/role-label claims. A three-state scoped
native control supplies nonstuttering Next/closing failures; it receives no
extra oracle agreement credit. The original `Live` versus derived `Identity`
claim test remains explicit. A 392-setting budget matrix admits only an exact
complete artifact or typed limit refusal. Additional
edited group/tag/bin, action, pending/node/term/work/depth, effectful/opaque
capture and invalid retained-ID controls refuse explicitly without callback
output. Formula/product defaults are unchanged. See ADR 0027 for the precise
arena/node/work bounds and separate depth limit; no aggregate byte budget or
strict integrated-liveness claim is made.

Reproduce after fetching the pinned bundle into a private cache:

```sh
python3 -I conformance/probes/tlc-tableau/capture.py \
  --java <pinned-java> --javac <jdk17-javac> --jar <verified-pinned-jar> \
  --reason "regenerate complete particle/tableau selection"
python3 -I scripts/check-tlc-tableau.py
cargo test -p dive-liveness --test tlc_tableau --locked
```

The hash verifier covers the complete current and historical selections and
raw bytes, including failures, and checks the native source matrix. The Java
JDK/compiler requirement is local diagnostic tooling, not an added runtime
dependency. No hand-edited golden files or corpus verdicts are introduced.
