# Real pinned queue suspension controls

The producer calls the unchanged pinned `DiskStateQueue`, including its inherited
final `StateQueue.suspendAll`, `resumeAll`, and synchronous dequeue methods.
The jar SHA-256 and exact Java commands are recorded in `capture.json`.

The complete selection is three scenarios, widths 1/4/8 and eight repeats (72
observations). Normal suspension waits for every actual worker to reach the
queue barrier; no queued state is removed before resume. After resume each
worker takes one state. Finishing before suspension returns false. Finishing
while the actual controller is waiting also returns false, and released workers
take no states. Each controller/worker future has a fixed five-second bound;
missing suspension or worker cleanup raises a failure rather than returning a
claimed observation.

Reflection only observes the private stop/waiting fields under the queue's own
monitor. It never sets them. Real queue methods create every observed state.
The selected empty TLC states do not exercise model evaluation, disk spilling,
checkpoint persistence, arbitrary interleavings, ordinary clock scheduling or
model-checking verdicts. The observer controls when workers enter the queue and
when completion is requested; these are selected lifecycle controls, not an
unrestricted scheduler proof. Joined native source waves remain a separate fact.

`stage0` preserves compilation, `stage1` the complete raw runtime streams.
`verify.py` performs independent selection/field/artifact readback. No output
row is removed because it would fail an expectation.

The additive `QueueEmptyProbe` selection retains 24 more controls at the same
widths/repeats. No initial state is enqueued. A worker observing actual empty
queue EOF calls `finishAll`, as a terminating worker would; every worker exits,
and the pending controller returns false. This does not execute `TLCWorker` or
prove every interleaving of its model evaluation. The original 72-case receipt
and raw source remain unchanged. `empty-readback.json` retains the additive
source/class/streams and exact selected rows separately.
