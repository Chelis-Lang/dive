# Native DFID controls

The primary reference is the **unmodified** pinned jar at source revision
`94d0c5070852b37b801bb8998c436383ab7ee37b`. `oracle-controls.json` records
159 selected clean controls and all raw/spec/configuration hashes. There are
no trace-dump flags or RNG injections. Keep worker-selection ambiguity explicit:
CLI seed/aril do not seed DFID workers (D-002). Bounds 0/1 and a zero exit without
completion are evidence, not proof (D-018). `DfidRootsMixed` measures D-019's
truncated cross-root word.
Primary capture directs TLC metadata into a temporary directory and records
that transient path as `<temporary-metadata>`; no state-store files belong in
this fixture directory.

`SeededDfid.java` is a **separate secondary reference**: only worker selection
seeds are injected after worker construction, before start. It preserves the
pinned visitation algorithm, and supplies the CLI-equivalent enumeration seed 7.
`secondary-controls.json` records 168 controls, adapter/compiler/JRE identity and
all raw hashes. These controls test exact deterministic search fields without
masquerading as an unmodified oracle. Native tests run them in both storage modes.

Use an isolated verified cache and the pinned Java runtime. For secondary runs,
provide JDK17's compiler separately. The environment variables below name those
executables/jar; no system Java is assumed.

```sh
python3 -I crates/dive-check/tests/fixtures/dfid/capture.py \
  --java "$DIVE_JAVA" --jar "$DIVE_JAR" --reason "documented regeneration reason"
python3 -I crates/dive-check/tests/fixtures/dfid/capture-secondary.py \
  --java "$DIVE_JAVA" --javac "$DIVE_JAVAC" --jar "$DIVE_JAR" \
  --reason "documented secondary regeneration reason"
cargo build -p dive-cli --locked
python3 -I crates/dive-check/tests/fixtures/dfid/compare.py \
  --dive target/debug/dive --report conformance/reports/dfid-primary-controls.json
cargo test -p dive-check --test dfid -p dive-api --test dfid --locked
```

The report accounts every primary control; it does not turn unstable references
or unverified binding classes into agreement. Scope: pure finite/bounded safety,
one worker. Live checker state, effects, temporal, VIEW, symmetry, ALIAS,
postconditions, continuation, checkpointing and constrained initial/action-property
witness contracts are refused by name. Raw printing and DFID collision figures
are not certified. Complete family acceptance remains issue 118 / F-025 audit-required.
See the [independent review](../../../../../docs/reviews/2026-10-10-native-dfid.md)
for the separate graph, witness and captured-context controls.
