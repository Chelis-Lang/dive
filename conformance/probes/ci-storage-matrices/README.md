# Complete storage test matrices in CI

At PR #226 head `c0d8e1a`, GitHub CI run `38172248951` timed out in
partitions 2 and 6 after 15 minutes. The logs are retained here. Both jobs
completed compilation; large checkpoint/storage matrices dominated test time.
Other checks passed. The failed head was not merged.

The same 34 matrix tests are divided into 336 independently schedulable tests.
All 768 original inner cases remain: 192 structural-storage combinations, 432
native spill/recovery combinations, and 144 VIEW checkpoint combinations.
The original files and a token comparison receipt preserve every helper body,
assertion and source/profile/store declaration. Only supplied loop slices and
private temporary-path uniqueness changed. Production Rust, timeouts, worker
widths, spill limits, witness mutation controls and required checks are unchanged.

`selection.json` declares the original cross-products. The normal
`scripts/check-test-partitions.py` census checks their actual compiled test names,
rejects nine corrupted selections; at producer `d21ea2c6` it proved eight partitions were
disjoint and cover every non-ignored workspace test. This is CI scheduling
maintenance; it does not expand supported model-checking behavior or parity.
The earlier 1,346-test qualification retains its own producer and receipt;
the new inventory contains 1,648 tests because 302 test groupings were split.

## Closed local qualification

Test producer `d21ea2c6` passed all 64 literal Python commands in its frozen
CI plus eight Rust gate stages: 1,648 tests passed with zero skips, followed
by doctests, warning-denied documentation, layering, standalone trace and
dependency checks. [The retained qualification](epochs/d21ea2c699c6514f61038c6fe8205219b83fa2f5/index.json)
contains the selected 855-input source packet and hashed command logs. An
independent readback compares captured bytes with Git, checks the full matrix
inventory and rejects eight rehashed semantic corruptions.

Production Rust, Cargo/toolchain inputs and all 70 literal include sites
remain identical to runtime producer `05b3a1ef`; only the three named test
files differ. The existing model runs retain that original producer. This
test qualification makes no fresh runtime, family-parity or hermetic-build claim.
The auxiliary focused Clippy log and unformatted split receipt are explicitly
preparatory; the frozen full gate and final source packet qualify this source.

## Sixteen-partition CI continuation

Head `67719999` passed seven of eight test partitions, the workspace checks
and conformance; partition 3 completed 109 of 226 selected tests before its
15-minute job limit. The retained log includes the unfinished queue. Several
checkpoint/filesystem tests took substantially longer on the Linux runner than
on the local machine. No assertion failed.

CI now distributes the same complete inventory across sixteen jobs. The census
checks the actual workflow matrix and nextest denominator as well as disjoint
coverage and all nine matrix corruption controls. Job and per-test timeouts,
case IDs, assertions and original matrix selections are unchanged. This is a
scheduling change; fresh gate and CI qualification remain required.

## Closed sixteen-partition qualification

Frozen scheduling producer `3dcabae1` passed all 64 literal CI Python commands
and all eight Rust gate stages. All 1,648 workspace tests passed with zero skips.
The actual nextest census selects all tests exactly once across sixteen
partitions and preserves the 336 storage test groups. It rejects nine matrix
mutations and three corrupted workflow declarations.

The [separate epoch](epochs/3dcabae15a7dbfcbd093cc1eb654beeaf693addf/index.json)
retains the 857 selected inputs, all 70 literal fixture sites, command logs and
independent readback. Eight rehashed semantic corruptions are rejected, including
missing commands, incomplete partitions, skipped tests and runtime retagging.
Rust/Cargo/toolchain inputs are identical to the earlier test-splitting producer
`d21ea2c6`. Existing model runtime campaigns keep their `05b3a1ef` producer.
This is CI scheduling qualification, not a new model run or feature certificate.
The pre-run declaration's pending text is retained in the frozen source packet.

