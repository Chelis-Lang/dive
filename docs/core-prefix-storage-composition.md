# Composed core delivery

This receiving epoch combines request-owned periodic service and candidate
reconstruction, opt-in disk seen-state safety checking with positioned replay,
and bounded PlusCal procedure checksum editing. Use the functionality guides:

- [Disk seen-state safety](user/disk-seen-safety.md) describes retention,
  ordered failure reporting, cleanup and independent replay.
- [Request-owned service](request-owned-periodic-service.md) and
  [producer-owned candidates](producer-owned-temporal-candidate.md) describe
  Rust diagnostics. These APIs cannot establish ordinary periodic model proof.
- [P elsif](pluscal-p-elsif-checksums.md) and
  [multiple procedure checksums](pluscal-multiple-procedure-checksums.md)
  describe bounded Full editing and executable CLI/Rust examples.
- [Inductive tuple replay](inductive-tuple-trace-replay.md) describes the
  separately proved variable-shape subset; general saved stacks still refuse.

## Verification boundary

The immutable source producer is `05b3a1ef`. Its selected archive contains
844 inputs, including all 479 compiled Rust sources and all 65 fixture inputs
at 70 literal include sites. The full local gate passed 1,346 workspace tests
with zero skips. All 71 originally declared commands passed; a separately
audited CI supplement also passed the inherited inductive-tuple guard. The
combined command set covers every literal Python command in its frozen CI.

Fresh release executables were built from that same source. The selected
campaigns compare memory and disk checking with preserved primary references,
replay pure positioned invariant failures, retain effect-dependent replay
refusals, test request/candidate authority, run the checksum campaigns and their
negative controls, and execute the user guides. All 21 CM1 and 16 generated
ValidationTestSuite models agree at widths 1, 4 and 8.

The [retained epoch](../conformance/probes/core-prefix-storage-integration/epochs/05b3a1ef498ea1609c0413383b81bebb1b411749/index.json)
contains hash-indexed source, command logs, runtime receipts and failed attempts.
Read it without Cargo using `python3 -I scripts/check-core-composed-epoch.py`.
Its corruption controls deliberately rehash altered packets, then check that
failed commands, skipped tests, missing fixture/CI coverage, false witness
validation and missing worker selections are still rejected.

The earlier `2bff2728` epoch and each component producer remain separate. A
fixture-path mismatch in the first fresh disk campaign is retained; the corrected
adapter asserts the original absolute paths before rebasing only file/config
metadata. It changes no verdict, count, storage, diagnostic or witness assertion.
The original failed offline invocation and Git-status interruption remain too.

This is a selected source and runtime archive, not a hermetic build or an
authenticity signature. Executable hashes are retained; large local Mach-O
executables are not bundled. The final documentation/reader packaging does not
change the qualified Rust/Cargo or literal fixture bytes. Ordinary temporal
scheduling, general procedure-stack replay, recovery/distributed interactions
and complete TLC semantic families remain uncertified. No family or overall
percentage is promoted by these selected passing runs.

## CI test scheduling

The first PR head timed out in two test partitions after compilation. The
[retained matrix census](../conformance/probes/ci-storage-matrices/README.md)
records those failures and the subsequent test-only split. All 768 original
cases and their assertions remain; 336 smaller tests replace 34 large matrices.
This improves work distribution across the existing eight CI jobs without
changing production behavior, test bounds, or any conformance requirement.
The original 1,346-test epoch above remains tied to its original source.

The subsequent frozen test producer `d21ea2c6` passed the complete local gate:
all 64 CI Python commands and eight Rust stages, with 1,648 tests and zero skips.
Its [separate retained epoch](../conformance/probes/ci-storage-matrices/epochs/d21ea2c699c6514f61038c6fe8205219b83fa2f5/index.json)
contains source, logs and independent readback controls. Production Rust,
Cargo/toolchain inputs and all literal fixture sites match the original runtime
producer. The 41 existing runtime campaigns retain `05b3a1ef`; splitting tests
does not re-execute or retag those campaigns.

The initial eight-job CI split subsequently passed seven partitions but timed
out in the remaining partition on a Linux runner. The current CI declaration
uses sixteen disjoint partitions with the same complete tests and unchanged
timeouts. The retained failed log and scheduling contract are in the
[CI matrix record](../conformance/probes/ci-storage-matrices/README.md).
This scheduling continuation requires its own qualification; the earlier
`d21ea2c6` gate and `05b3a1ef` runtime receipts retain their producer identities.

The frozen sixteen-partition scheduling producer `3dcabae1` has now passed
all 72 local gate commands, including all 1,648 tests with zero skips. Its
[separate receipt](../conformance/probes/ci-storage-matrices/epochs/3dcabae15a7dbfcbd093cc1eb654beeaf693addf/index.json)
records the complete, disjoint inventory and independent corruption controls.
The existing native runtime and test-splitting epochs keep their own producers.
