# Actual local progress/queue method controls

Issue #201 / [ADR0059](../../../docs/decisions/0059-producer-progress-requests.md).
All 33 selected rows (11 cases at widths1/4/8) remain in `capture.json` and
`primary.tar.gz`; no timeout or error row is dropped. Jar SHA-256 is
`7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d`.

`ProgressProbe` constructs actual FastTool, ModelChecker, local Worker threads
and the original real state queue. A deliberately instrumented Java
`Progress.Gate(v)` is installed through TLC's actual native-module override
mechanism; it blocks the first actual Next predicate for controlled injection.
This is explicit instrumentation, not a claimed pure model operator. The queue
wrapper delegates unchanged checker suspend/resume/finish calls. Workers retain
the original queue itself. Private error/keepCallStack fields are read only.

`AbstractChecker.runTLC`, `ModelChecker.doPeriodicWork`, local `Worker.run`, actual
queue suspension and final `liveCheck.finalCheck` execute unchanged. Natural
rows do not force a request. Controlled rows inject force during Next and release
that gate after actual suspend entry; finish-during also explicitly finishes the
actual queue. These are controlled method/model projections, not native
ordinary-clock/count/model certification. Distributed TLCWorker is excluded.

Measured disposition by case (all three widths):

| Case | Original observed boundary |
|---|---|
| natural / final-eof | Normal runTLC0, workers joined, final check2116 |
| natural-safe | Normal runTLC0, workers joined, final check0 |
| resume | Forced partial2116 returns with the actual width of workers alive |
| resume-safe | Actual resumeAll then normal runTLC0/final0 |
| false-hook | Periodic2104 returns with workers alive |
| hook-error | Actual EvalException2179; no invented success/exception class |
| worker-error | runTLC0 but retained original2179 and keepCallStack=true; no final-success projection |
| finish-during | Actual suspend returns false after finish; normal cleanup |
| final-violation | Forced2116 returns with workers alive |
| failed-init | Actual doInit2107; no startWorkers |

Later `cleanup-joined` is explicitly probe-owned finish/join after the original
return; it cannot be attributed to runTLC. A partial candidate's stuttering word
is not declared wrong merely because Next could advance: `[Next]_vars` permits
stutter without fairness. Raw behaviors and exact counters remain available;
there is no claim that these volatile schedules equal the native wave schedule.

`primary-index.json` hashes the closed artifacts, sources and all four failed
development epochs. Earlier proxy attempts failed because Worker requires a
concrete Tool; these source/error rows remain archived. Epoch3 retains the first
complete18 method rows, and epoch4 retains the33 rows before explicit retained
error/keepCallStack projection. The failed focused empty-Init assertion/log and
its minimal source are retained separately: empty Init is not an Init error.

The [user guide](../../../docs/producer-progress-requests.md) and Cargo example
exercise genuine configuration-owned native requests. Fresh candidate/EOF
replay, wrong word/claim/ending controls and unchanged graph comparisons remain
separate from these reference timing observations. Original resource caps and
ordinary checker/checkpoint refusals remain; F-023 gets no family certification.
