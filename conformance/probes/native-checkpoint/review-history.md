# Review and validation history

The primary oracle transcripts and their hashes remain unchanged. The failed
recovery-directory attempt is preserved separately from the successful run.
Implementation admission bounds and native controls do not modify TLC goldens.

Before the final gate, a focused compound-value control initially failed at
fixture parsing. Its model-value template contained `Init == x=<<A,[tag |-> B]>>`.
The analysis diagnostic was `UnexpectedToken`, expected a module unit, found
`A`, range 64..65; the test failed before checkpoint execution. Whitespace was
added around the assignment/tuple tokens in that new test fixture. The unchanged
lossless resume assertions then passed. No implementation or oracle assertion was
changed to obtain that pass.

Read-only review found unchecked addition in imported initial/generated counter
consistency. The arithmetic now refuses overflow; the coherent-checksum negative
matrix includes maximum-u64 counters and requires a refusal without a receipt.
Review also found that failed staging inspection could replace the original
write/commit error. Focused injection controls now require the original error to
remain alongside inspection/cleanup failures; successful cleanup still returns
the original failure.

The checkpoint slice was developed from the frozen predecessor delivery, before
frontier storage was integrated on the shipping branch. Integration must reject
both transient budget/directory families before file/model effects, preserving
this slice's narrow admission contract. Wider spill, graph, runtime and parallel
recovery remain explicitly pending.

The first full gate was stopped during compilation, before tests, to add compiled
public API rustdoc examples. All its child processes were stopped before editing.
The final gate runs the subsequent frozen source, including those examples.

The second gate was stopped before tests after review identified a retained
counter overflow. `before-counter-repair.log` preserves the coherent-checksum
maximum-u64 archive reproduction: continuation panicked in the unchecked generated
increment and returned an internal error. The exact source bytes/hashes and test
binary hash are retained in `before-counter-repair-source/` and its JSON manifest.
Native admission now caps counts at the largest positive TLC long, and continuation
uses checked increments. Maximum/near-maximum-u64 negative controls require a
pre-binding resource refusal, zero worker use, no statistics/effects and no receipt.
Parallel bulk-count arithmetic cannot receive native restored counts: this slice
always uses the reported inline engine; broader parallel recovery remains refused.

The final fixed source passed the complete workspace gate (727 tests), followed
by all 576 recorded native release controls and CM1/ValidationTestSuite smoke at
1, 4 and 8 workers. No oracle assertion or generated golden changed.
A read-only scratch reproduction separately demonstrated that malformed public
IR variable metadata can panic before ordinary/checkpoint admission, including
the panic-handler metadata construction. Both original and explicit-extern
reproduction source/log/binary hashes are preserved in `malformed-arena-review/`.
That pre-existing endpoint defect is tracked as issue #157 and will be repaired
in a fresh follow-up rather than editing this frozen checkpoint gate.
