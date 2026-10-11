# Ordered worker-stop service evidence

These are raw pinned observations, not edited goldens or ordinary proof receipts.
The primary jar SHA is `7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d`.
Each complete 60-row selection preserves every model/config, invocation, raw
stdout/stderr, actual process exit, timeout and payload SHA. The controlled
observer calls actual `int AbstractChecker.modelCheck`; its subprocess exit does
not substitute for that integer. Actual queue/livecheck methods delegate to the
original implementation; observer cleanup occurs separately after method return.

- `primary-development0`: original 60-row selection. Bare action-property and
  constant FALSE Next have binding rejections; these are not runtime support.
- `primary-rejected1`: all 60 rows retain the invalid `[](x'=x)` action formula
  setup rejection, including otherwise unused declarations. No runtime credit.
- `primary-rejected2`: corrected boxed action `[ ][x'=x]_<<x,y>>` (without space
  in actual source) preserves six remaining constant-FALSE Next rejections.
- `primary`: corrected full selection. The deadlock relation has actual state
  guard `x<0` and a complete syntactic action; it runs but produces no successors.
  Widths 1/4/8 × ten cases × observer/plain CLI are all retained. Exact returns
  include2179/255 Next division,2110/12 invariant,2112/13 action property,
  2109/75 incomplete,2114/11 deadlock,2103/75 failed Init and0/0 safe/empty Init.
  FALSE hook cases retain original Next/invariant error. Exactly one accepted
  error and later rejected attempts remain visible, along with possible actual
  graph writes after the winner. These controlled observations are not native
  graph, queue, clock or count equivalence.
- `development-focus`: original compile and invalid generation/suspension test
  assumptions are retained beside corrected controls. These logs without a
  complete archived input freeze are development evidence only.
- `declared-native-selection.json`: preimplementation inventory; fresh closed
  producer/runtime evidence must account for all actual selected outcomes.

See [the executable user guide](../../../docs/stopped-periodic-service.md) and
[ADR 0078](../../../docs/decisions/0078-stopped-queue-worker-errors.md). No ordinary
Holds, scheduling, whole-family or model-proof certificate is established here.
