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
