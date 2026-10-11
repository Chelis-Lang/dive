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
