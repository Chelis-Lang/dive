# Tuple-only independent replay

The implementation and runnable CLI/Cargo contract are in
[the user guide](../../../docs/tuple-only-trace-replay.md) and
[ADR 0064](../../../docs/decisions/0064-tuple-only-independent-replay.md).
`runtime_names.rs` exercises actual and forged words, native and lossy transport,
resolved aliases, wrappers, assumptions, nested string construction and variable
stack refusal. A closed native list refuses unclassified constructors rather
than treating purity as proof that no names can be created.

`before-implementation-original` and `before-implementation-corrected` retain
separate, complete 30-consumer observations from the preceding immutable
binary. Each has six fresh pinned TLC violations and 24 independent native
replays. The original concat row mislabelled its original and altered values;
its raw result remains unchanged and receives no agreement credit. The second
epoch corrects the selection itself. Both precede this implementation: neither
is evidence that the new strict admission ran.

`before-fixture-repair` retains the closed 12-pass/1-failure focused run and nine
explicitly selected source inputs. Its variable-stack fixture was malformed
(`s=<<...>>`); that row never reached the admission proof. The corrected fixture
retains the strict Unsupported expectation. This archive is a partial source
selection, not a full workspace build attestation or a passed gate.

Run `python3 -I scripts/check-tuple-only-replay.py` to check these distinctions.
Original command paths in archived drivers describe the producer machine;
use the user guide for current portable invocations. Current implementation
validation is run anew against its own source and binary identity. No family is
certified and source-variable saved stacks still need an inductive shape proof.

## Implemented epoch

`implemented-epoch` retains the original frozen producer, its full source hash
index, 945 portable compilation/contract inputs, all eight local gate logs and
all 582 runtime consumers. The full gate ran 1,212 tests with no skips. Eight
tuple-operand cases each ran both profiles, both stores, workers 1/4/8 and chaos
0/13: 192 real invariant violations, 192 Valid original words and 192 Invalid
altered words. All 21 CM1 and 16 ValidationTestSuite models fully agreed at
workers 1/4/8. The Cargo guide compiled and ran against that producer.

The preceding full-gate attempt exhausted filesystem space: all 1,212 tests
ran, 1,171 passed and 41 failed. Its original logs and disposition remain under
`storage-failed0`; the unchanged-source rerun supplies the green evidence. No
test, expected result or time limit was weakened. Archiving and this verifier
were added after all runtime consumers closed, so their delivery commit is not
retagged as the original runtime producer. The full source index covers 70,821
inputs; portable compilation sources are a separately identified subset, not
a second claim that every historical payload was copied here.
