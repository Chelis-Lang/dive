# 0004: Worker count, search depth and counterexample length

- **Status:** accepted
- **Date:** 2026-10-08

## Context

Soundness contract rules 1 and 2 ask `dive` to match TLC's distinct-state count, generated-state count and search depth, and to produce safety counterexamples of the same length as TLC's ("both are shortest by breadth-first search"). Rule 7 requires worker-count invariance.

TLC does not provide that invariance:
- With more than one worker, TLC's search is only *approximately* breadth-first. See Lamport's comment in `tlc2/tool/TLCTrace.java` (lines ~99–110).
- The reported depth varies with worker count. This is acknowledged in the TLC usage text (`TLC.java`) and in tlaplus/tlaplus#883.
- The Examples CI deliberately skips comparing `stateDepth` for this reason.
- Distinct and generated counts for *completed* exhaustive runs do not depend on worker count. The Examples CI relies on this.

## Decision

1. Goldens that record depth, counterexample length or a counterexample trace come from TLC run with **`-workers 1`**. That is the only configuration where TLC's BFS is exact.
2. Distinct and generated state counts may be compared against TLC runs at any worker count. The harness's worker-invariance mode (for example `--workers 1,4`) measures and reports TLC's own invariance, so any TLC variance is recorded as an oracle fact and not blamed on `dive`.
3. `dive` is deterministic at every worker count: same counts, same depth, same chosen counterexample. Its counterexample is a shortest one, compared with the single-worker TLC golden by length and validity (rule 2), not byte for byte.

## Consequences

Generating goldens is slower, because it is single-threaded. That cost is acceptable since goldens are generated rarely. Multi-worker TLC runs remain useful as invariance probes.
