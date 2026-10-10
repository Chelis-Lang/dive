# Pinned bounded final SCC and witness evidence

This family preserves every selected model and failure. Its explicit final-only
Rust library path constructs actual ordered graphs and uses pinned SCC/greedy
witness selection; ordinary strict CLI liveness remains refused.

`selection.json` contains the previous 26 execution controls plus four added
state-recurrence, action-recurrence, fairness-cycle and safe-fairness cases.
`capture.json` retains 19 completed references and 11 reference/helper failures.
`native-outcomes.json` retains all 120 model/width outcomes: at each of 1/3/4/8,
18 unvalidated backend candidates, one final-only holds result and 11 binding/
refusal/error outcomes. No reference failure earns error-class agreement credit;
none of these rows adds ordinary strict corpus agreement credit.

The Java helper invokes actual pinned `LiveWorker.checkSccs`, `checkComponent`,
`dfsPostFix` and `bfsPostFix`, and actual graph `getPath`. Observing graph methods
log original first visits and component pop batches. The copied graph preserves
final roots/adjacency, not original disk offsets or write history. Suppression
of private-method trace printing does not replace the methods' decisions.
The original unmodified `LiveCheck.finalCheck` then runs on its original graph,
preserving actual configured attribution and printed prefix/cycle/stutter streams.
The helper process exit is not the TLC verdict; `final_check_code` and original
message streams are distinct fields.

Rust field comparisons retain anchors, component/pop order, first visits,
postfix endpoint/stack, fingerprint closure and reverse product prefix at every
width. Original Action/configured `Live` and derived role `Identity` stay distinct.
An independent Floyd reachability/set acceptance and path/edge-label checker
covers 4,096 three-node graphs. Fresh `dive-trace` binding replays every supported
candidate in memory and JSON and rejects changed initial states, Next/closing
edges, loop targets and omitted claims. The API certificate is private; supplied
graphs and backend flags cannot attest to model validation.

Existing entry/cell/inspection bounds remain fixed, with named preallocation
and witness-position refusals. They do not account aggregate bytes. Exact-store
proof is refused before execution because the temporal graph uses fingerprint
identity; no forced-collision graph earns exact proof credit. Foreign IR/name
ownership, effectful/opaque contexts and malformed labels/identities are refused.
Safety stopping facts, speculative work, recursive enumeration and SCC/witness
work stay separate. Initial replay, accepting-sink shortcuts, complete error
classes, periodic schedule, symmetry/VIEW/ALIAS and host/effectful execution are
not certified by this family. F-019/F-020/F-021/F-022 remain partial, F-023 missing.

Reproduce the actual Java selection with the pinned verified JAR and a JDK 17:

```sh
python3 -I conformance/probes/tlc-final-check/capture.py \
  --java "$DIVE_JAVA" --javac "$DIVE_JAVAC" --jar "$DIVE_ORACLE_JAR" \
  --reason "Review bounded pinned final SCC/witness controls"
cargo run -p dive-liveness --example tlc_final_report --locked > final-native.json
python3 -I scripts/check-tlc-final.py
cargo test -p dive-liveness --test tlc_scc --test tlc_final --locked
cargo test -p dive-api --test tlc_temporal --locked
```

Before replacement, preserve each original manifest/source/raw tree. Original
SCC-only controls, the failed missing-cache attempt and early final-only helper
runs remain under `historical-*`. Every previous current source matrix and exact
producer is archived as `historical-before-final-check` in the five preceding
families; it describes that original source, not this increment.
