# Read-only implementation seam for #209

Current `Plan.blocks` uses `(StmtId, index)`, where a one-branch If uses 0/1 for
Then/Else. `Writer::if_branches` emits P flat branches as nested reference If ASTs,
but reuses those same keys. Before admission, the virtual branch ordinal needs to
be part of ownership: reserve `2*ordinal` and `2*ordinal+1` for each Then/Else
vector (or an equivalent typed key), retaining the old single-branch behavior.

A remaining Else chain is a virtual If node, not a flattened concatenation of
source branch statements. An overlay node can refer to `(source If, branch
ordinal)` while preserving source tests and exact byte indentation. If an outer
Then transfers and the complete remaining Else chain falls through, append the
UpdatePC assignment after that complete virtual If node, not independently to
each of its source branches. Primary after-generation stored AST is authoritative;
direct Explode is ownership evidence rather than a substitute header root.

Build the chain from its last branch toward its first, accumulating needsGoto,
without repeated whole-arena transfer scans or branch-list cloning. Reserve the
virtual wrapper/vector/key/continuation work before constructing it, preserve the
existing one-time transfer index, recursive depth and byte writer ceilings, and
carry the unchanged generated-visit flag through nested While tails. Source
branches without transfers are still source nodes unless actual pinned mutation
requires their shared container to change.

This note is a design proposal only. No current source, CRC policy, refusal,
claimed census or frozen producer has changed. Source and native proof will be
implemented after #203 complete gate/readback release.
