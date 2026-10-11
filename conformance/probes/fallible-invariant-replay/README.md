# Positioned fallible invariant-error replay (#217)

The clean `primary/` runs use the pinned primary jar SHA-256
`7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d`, seed 7,
fp 0 and one worker without trace-dump instrumentation. Command records pin
source/config bytes. This directory is distinct from the original
[wrong-count and guarded epochs](../fallible-seen-engine/invariant-replay-before/).

| Model | Exit | Generated | Distinct | Queued | Depth |
| --- | ---: | ---: | ---: | ---: | ---: |
| ReplayMembership | 76 | 3 | 3 | 1 | 2 |
| ReplayNew | 76 | 2 | 4 | 2 | 2 |
| ReplayLaterViolation | 76 | 2 | 3 | 0 | 2 |
| ErInvDivZero | 76 | 4 | 5 | 2 | 3 |
| ErInvReplayViolation | 12 | 3 | 3 | 1 | 2 |
| ErInvReplayError | 75 | 4 | 5 | 1 | 3 |
| ErInvReplayProp | 76 | 4 | 4 | 1 | 3 |

`ReplayLaterViolation` actually reaches the later invariant violation during replay:
its new key is admitted, but it is not logically queued and the original error remains.
The last case is a private bound-engine control; public configured PROPERTY
still refuses before binding. The invariant-violation control stops before the
replay boundary and must remain an ordinary positive, not invented replay
credit. Original errors and second-error class selection are separate from
independent violation validation. Resource-limit words are diagnostic only.

Development controls exercise actual membership/insertion file corruption,
entry-bound exhaustion after an admitted replay key, original error/position
retention, late cleanup and the impossible duplicate-insert protocol seam.
The first targeted run retained 6 passing controls and 2 old-refusal expectation
failures. The final targeted run passed 14 unit controls (26 unrelated filtered), all
16 disk integration controls, the public API control and the actual four-row
Cargo example; scoped Clippy also passed. The earlier public API assertion used
`stats` instead of the existing `statistics` JSON field; its failure is retained
without any production change to obtain a pass. Development logs explicitly
carry no frozen-producer qualification. These controls do not replace a complete
source-frozen gate or runtime campaign.

Current-map guards first rejected changed engine/API/ledger hashes. All 31
original parent authorities/docs are archived separately; only current source
indexes were refreshed. The second offline pass closed all 42 checks green.
Original runtime/gate receipts and old wrong-count/refusal rows were not retagged.

The immutable producer `881b75992d0ef5dc11539018da526e5e55066027` closed all
eight gate stages: 1,284 tests passed, zero skipped. Its original memory/disk
selection has 384 unchanged memory rows, 576 disk rows and 64 references; the
new replay selection retains 104 references, 432 pure disk rows and 504 explicit
Print-output refusals. Fresh independent controls retain the entire original
named selection: 144 Valid words, 864 Invalid mutants and 36 actual VIEW rows.
CM1 21 and VTS 16 agree at requested widths 1/4/8. Both actual Rust guides ran.
See [the immutable packet](current/delivery-index.json); no new family certification is
claimed. See [ADR0072](../../../docs/decisions/0072-fallible-invariant-error-replay.md)
and [the runnable user/API guide](../../../docs/user/disk-seen-safety.md).

## Print output boundary (pre-review source bd5841b95)

`ReplayPrint` adds `PrintT("expand")` to the next action. Clean primary exits
76 with 2 generated/4 distinct/2 queued/depth2 and emits three events; the
pre-review targeted native API run emitted one while reporting known class
2111. That failed output claim is retained separately as development evidence,
not an immutable compiled producer receipt. No old primary/run is retagged.

Seven clean probes retain the same counts and exit. Actual event counts:

| Probe | Primary Print events | Placement |
| --- | ---: | --- |
| ReplayPrint | 3 | next action |
| ReplayPrintInit | 1 | Init only |
| ReplayPrintEmpty | 1 | empty string in Init |
| ReplayPrintInvariant | 4 | invariant |
| ReplayPrintWrapper | 1 | cached wrapper |
| ReplayPrintAlias | 3 | configured native PrintT alias |
| ReplayPrintReplayOnly | 2 | later invariant reached by error replay |

The corrected route uses an actual monotonic evaluator event flag, not rendered
text. All seven fixtures × both profiles/stores × requested 1/4/8 widths refuse
the unproved output contract, retain original error and counts, and report
actual inline execution/cleanup. A raw empty-text event under quiet capture
sets the flag with zero sink calls; warnings and an unused Print definition do
not. An earlier positioned membership/read fault remains the first resource
cause instead of being replaced by the output refusal. Memory behavior and pure
replay are unchanged. Init-only/cached events are conservatively refused even
where that particular reference event was not repeated. These targeted
controls are included in the completed frozen gate and fresh executable campaign.

## Integrity, failures and timing

Run `python3 -I scripts/check-fallible-invariant-replay.py` for portable packet
readback. Optional `--binary-root DIR` verifies all five retained actual
executables; `--source-root DIR` compares the original input bytes. The source
archive contains 1,226 tracked Rust/Cargo/toolchain/configuration files, including
historical and fixture Rust; it is deliberately broader than an exact compiler
dependency inventory. The final metadata head does not retag the original
runtime or gate producer. SHA-256 checks integrity, not authenticity or proof.

The first same-source gate stopped at ENOSPC during test inventory compilation;
its complete receipt/log is retained. After no-reader checks and inventoried
cleanup of inactive incremental objects, the unchanged producer passed. Total
local eight-stage wall time was 926.393 seconds; nextest stage wall time was
547.630 seconds, with a test summary of 475.218 seconds. This is the unsharded
local two-job/two-thread run, not a Linux CI performance claim; per-test bounds
were unchanged. Fifteen positioned fault/precedence controls also passed in a
separate filtered run (141 unrelated tests filtered).

The unchanged compiled witness consumer completed before an outer receipt
assertion incorrectly assumed six mutants per row. Both old and current
selections have 72 rows with six, 36 with five, and 36 with seven mutants. The
failed assumption is retained; readback instead proves equality of every
original named mutant and status, with all 864 preserved. No implementation,
consumer, oracle or test assertion was changed to obtain that equality.
An attempted focused orchestration without mandatory environment was rejected
before launching Cargo; its receipt is retained too.

A post-gate embedded-input audit retains an explicit original archive gap:
448 actual crate/xtask Rust source files contain 61 literal include sites with
56 distinct targets; 55 non-Rust targets were omitted from the original
selected index. The separate supplement qualifies original Git/live bytes,
without changing the frozen archive or retagging the gate. Readback independently
derives the complete literal include selection from the original archived Rust
and checks all targets. This remains a selected source packet, not a hermetic
build/toolchain/dependency archive.
