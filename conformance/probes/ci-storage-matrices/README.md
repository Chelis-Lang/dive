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
rejects nine corrupted selections, and still proves the eight partitions are
disjoint and cover every non-ignored workspace test. This is CI scheduling
maintenance; it does not expand supported model-checking behavior or parity.
The earlier 1,346-test qualification retains its own producer and receipt;
the new inventory contains 1,648 tests because 302 test groupings were split.
