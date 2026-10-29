# Compiled definition identity across INSTANCE scopes (#634)

The registered `core-instance-scopes` corpus holds the minimal plain Naturals
pair. Its official TLC goldens were generated with a reason before changing
compiler behavior. The original compiler falsely violated `ScopeHolding` and
incorrectly reported success for `ScopeViolation`, in both profiles on core and
auto. Holding must succeed at 3 distinct / 4 generated / depth 3; violation must
fail its initial `Inv` with 2107 / exit 12 and one row. The clean TLC initial
failure prints no counts/depth: the native raw 1/1/depth1 assertion stays
separate, with `reported_by_tlc=false`. The tests use the established ordinary
harness projection (`crates/dive-conformance/src/dive.rs`); no check or golden
was loosened.

`oracle.json` consumes nine additional official-harness facts. It retains exact
source hashes, corpus identity, oracle/JRE/classpath/library pins, reason and
harness source/binary identity. Verdict spelling is projected to the native
report spelling, with the original oracle verdict retained separately. The
GuardError cause is parsed from the clean nested tool message, not the trace
pass. Raw reports and failed controls stay outside the repository.

To regenerate, stage each config/module with the four helper modules in its own
Examples-layout model directory and manifest, using the repository's pinned
`oracles.toml` and a staging `corpora.toml` defining `core-instance-scope-controls`.
Run:

```sh
dive-conformance --repo-root "$STAGING" golden generate --corpus core-instance-scope-controls --tier all --no-sany --reason 'Pin nested INSTANCE cache-context controls for plain primed query and lexical demands in issue 634' --workers 1 --jobs 2 --timeout 20 --xmx 1g --raw-invocations "$RAW"
```

The guarded arithmetic body `IF v=v THEN 2*v ELSE 0` has a state dependency
visible to SANY. It is independently oracle-checked on the same reachable
integer values. This lets the unchanged base reference and validator check
these additional controls without hiding #515 in the original Naturals pair.
No #515 code is copied into this lane.

| Controls | Boundary |
|---|---|
| `GuardHolding`, `GuardViolation`, `GuardReordered` | Separate actuals for one shared source definition, both read orders and repeated re-entry. |
| `ScopedCalls` | Calls inside a lexical LET and a bounded quantifier. |
| `StepHolding`, `StepViolation` | Plain/primed cached definitions in one complete step; named outer actuals compile outside the inner substitutions. |
| `QueryHolding`, `QueryViolation` | Two nested identity-variable INSTANCEs with distinct constant bounds and supported ENABLED queries. |
| `GuardError` | A demanded error in the second actual must not disappear behind the first body's cache. TLC decides 2111 / exit 76, with underlying division-by-zero 2179 and 2 distinct / 2 generated / depth 2. |

`core_instance_scopes.rs` consumes every control in both profiles at workers
1/4/8 with memo, step-memo, eager aliases, folding and hash-consing ablations.
All required additional words must be present, validate independently, and
reject a forged initial row. Existing nonidentity/initial-query fixtures remain
explicit refusals.

The original Naturals pair is strict core/auto versus pinned TLC. Its separate
word controls use the exact frozen #515 candidate `82b03ca02270ba3d8e5e3944953e8fdfa03168d5`,
ELF SHA-256 `ad6666d16273789801ac7e077f433b02a6ef823943d3446eb0e45e90ce09786d`.
Those external receipts are source-qualified validation, not a claim that the
reference on this branch already implements that unmerged correction. Earlier
#515 raw evidence/goldens remain in their own lane; they are not duplicated.
