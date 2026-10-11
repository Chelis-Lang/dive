# Exhausted queue and terminal service evidence

These are raw pinned observations, not edited goldens or ordinary proof receipts.
All complete selections retain every input, invocation, stdout/stderr, timeout,
exit and payload hash. The primary jar SHA is
`7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d`.

- `primary-development0`: original complete 60 full CLI controls at 1/4/8 workers.
  The `safe-*` label used `<>(x=1)` incorrectly; initial stuttering is a legitimate
  violation. Constant FALSE Init and missing NEXT are binding controls, not EOF.
- `primary`: corrected complete 60 full CLI controls. Safe `<>(x=0)` runs exit0
  even with FALSE, integer, division or state hook bodies. Violations exit13;
  raw message code alone does not identify partial versus final selection.
- `outer-queue`: 60 controlled actual `ModelChecker.modelCheck` runs. The actual
  `IStateQueue` methods are delegated transparently from its testing factory
  before actual Worker construction. Reflection wraps the actual `ILiveCheck`
  implementation; it never supplies graph/results or replaces Worker objects.
  Actual finished suspension returns false before final selection in the finite
  selected controls. Observer cleanup is labelled separately after method return.
  Instrumentation is controlled evidence, not natural clock equivalence.
- `claimed-queue-controlled3`: 12 further actual outer/Worker observations at
  1/4/8 workers. A bounded latch holds one genuinely dequeued state until the
  actual `suspendAll` entry. Physical queue size0 with outstanding claimed work
  is not EOF: actual suspension returns true after successor generation. TRUE
  resumes after partial0 and reaches final0; FALSE/division stop without final;
  temporal2116 precedes FALSE. This controlled scheduler does not establish
  natural timing. `modelCheck` is void; observer process0 and reflected
  `errorCode0` are not `doPeriodicWork`'s internal2104 or full CLI success.
- `plain-cli-false-hook`: separate unmodified full CLI, unbounded `x'=x+1`, safe
  `<>(x=0)` and `_PERIODIC Hook` with FALSE. Declared512MiB heap/30s observer bound,
  width1/fp0/seed0. It exits10 after4.2456s, with3,913,590 generated/distinct and
  one queued state; no success or2104 message. These actual outer-exit facts do
  not replace the controlled method return or establish an upstream bug.
- `observer-compile-failed1`: attempted inheritance of final FastTool rejected;
  retained beside corrected candidate2 observation. Hook stage follows actual
  queue return plus pinned source; no fabricated hook callback.
- `word-compile-failed0` and `word-corrected1`: incorrect Rust Map construction is
  preserved. Corrected source rebinds original TLC stutter independently: memory
  and JSON Valid, with Init/claim/semantic-closing/ending-shape Invalid controls.
- Focused logs retain failed draft compilation and all subsequent selected
  outcomes. Development logs without a complete input archive receive no exact
  current-producer qualification; fresh gates establish delivery authority.

Re-run adapters with the verified pinned jar/JDK in an isolated capture directory.
Paths in raw commands retain the original invocation; no system Java is required.
The [user guide](../../../docs/terminal-periodic-service.md) and
[ADR0076](../../../docs/decisions/0076-exhausted-queue-terminal-service.md)
describe the narrower native diagnostic and its remaining obligations.
