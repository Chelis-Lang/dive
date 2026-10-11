# Source-qualified periodic backend controls (#189)

Pinned TLC jar SHA-256 is
`7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d`,
source revision `94d0c5070852b37b801bb8998c436383ab7ee37b`.
No golden files are edited here. Capture scripts execute the actual pinned Java
classes and retain every selected invocation, compiler stream, failure and raw
projection. They require explicit `--reason` and verify the jar hash.

`capture-growth.json` contains 36 selected actual `FastTool`/`LiveCheck` runs:
eight named models at default/final × forced/unforced (32 normal rows), plus four
controlled closed-disk failures. The original `development-growth32` source,
selection and raw first 32 rows remain unchanged. The producer follows bounded
actual source enumeration, `SetOfStates` slot insertion and original
`LiveCheck.check`, stopping at its first candidate. This is an explicit-prefix
backend projection, not a real `ModelChecker` wall-clock scheduling run.

Every normal row records `doLiveCheck`, actual pointer-table size and
`getSizeAtLastCheck` before/after every check. Null initial roots do not count as
pointer entries; forced empty checks can record zero. Tableau particle
registration and done-only buckets count once per fingerprint. A candidate
updates the processed OOS's snapshot, while a later OOS remains unprocessed.
The four controlled actual IO failures exit 1; a shutdown observer retains the
unchanged snapshots. They receive no native error-class or ordinary verdict
agreement credit.

`capture-tables.json` executes actual `DiskGraph`/`TableauDiskGraph` API calls,
including repeated writes, multiple particles per fingerprint, done-only and
undone buckets, zero snapshots and pointer restoration. It also contains all 30
strict growth comparisons across zero, equal-threshold, NaN and infinity cases.
These are pointer-table API projections, not model proofs.

Reproduce with supplied private-cache paths:

```sh
python3 -I conformance/probes/periodic-backend/capture-growth.py \
  --java "$DIVE_JAVA" --javac "$DIVE_JAVAC" --jar "$DIVE_TLC_JAR" \
  --reason "Reproduce complete source growth/snapshot selection"
python3 -I conformance/probes/periodic-backend/capture-tables.py \
  --java "$DIVE_JAVA" --javac "$DIVE_JAVAC" --jar "$DIVE_TLC_JAR" \
  --reason "Reproduce actual pointer-table/snapshot projection"
```

Focused Rust controls compare all 32 clean rows at workers 1/3/4/8 with chaos,
retaining exact before/after sizes, snapshots, growth flags, actual source stop
and private joined-wave metadata. Seven stopping candidates require fresh
independent word/memory/JSON replay; forged property, initial and closing controls
are rejected. Failed Init never produces an initialization-complete receipt.
Fixed graph work is shared across repeated construction/projection/checks.
Ownership/effects/width/budget controls remain explicit failures.

The [user guide](../../../docs/periodic-backend.md) and
[ADR 0051](../../../docs/decisions/0051-source-qualified-periodic-backend.md)
qualify supported and missing contracts. No partial Holds, ordinary clock
schedule, queue-suspension, native IO error-class, full-family or memory credit.
