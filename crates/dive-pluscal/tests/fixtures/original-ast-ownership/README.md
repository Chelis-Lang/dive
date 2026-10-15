# Genuine original-stage controls

Oracle: tlaplus commit `94d0c5070852b37b801bb8998c436383ab7ee37b`,
`tla2tools.jar` SHA-256
`7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d`,
pinned JRE archive SHA-256
`0b2b640e3046b64c8ec504de0ab9d91bb5610182bda21fad454681ce54d45a62`.

Actual arguments: `java -Xmx512m -cp PINNED_JAR pcal.trans FLAGS Case.tla`.
Original controls use `-writeAST` with optional `-debug`/`-label`; the exact
`AST.tla` bytes are consumed as `original_module`. WriteAST occurs immediately
after `ParseAlgorithm.getAlgorithm`, before name fixing or generation. Successful
retained writes exit255 because the pinned translation returns null. This is
artifact availability, not generated-module or filesystem parity in dive.

Debug translation controls use `-debug` with optional `-label`; all twelve rows
consume exact header/body CRCs and raw generated bodies. The three formerly pending
labelled If/Either/While cases now have exact native positives. The While control
preserves its original header bytes. A separately named procedural control adds
explicit EXTENDS Sequences and uses an actual fresh pcal.trans -debug invocation.
Its header/body CRCs are b8cfe3ef/5cd8f98a, with flat copied entry-vector ownership.
Generated-body comparisons ignore terminal translation-block separator newlines;
the pinned body CRC concatenates line bytes without LF/CR separators.

Every SANY run exits0 and reaches semantic processing/linting of Case with one
changed-since-translation warning. The nested-macro row also prints an embedded
missing-label validator error: SANY does not replay the translator's -label flag.
Consumed qualification fields preserve these stages. No row establishes clean
original PlusCal checksum validation or a model proof merely from exit0.

Four invalid source families retain primary failure disposition. Corrected label
modifier placement, nested-macro label insertion and C body braces have distinct
IDs; failures are not replaced or credited. Every fixture contains only data read
by native/API/CLI tests and is below 100 KB. Raw argv, timeout/exits, streams and
artifact bytes remain in external receipts; no AstDump output is used here.
