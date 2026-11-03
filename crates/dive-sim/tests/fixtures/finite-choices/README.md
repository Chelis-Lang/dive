# Finite random action contexts

The pinned primary captures in `oracle.json` use seed 7, one worker, two traces
and depth four. Both specs are consumed by API simulation tests. Raw stdout,
stderr and native TLC trace files are not committed; compact receipts bind
their exact inputs and outputs for pinned regeneration and external archival.

`let-action/Choices.tla` retains an uncached action LET: successive uses of its
random operator can differ within a row. `bound-action/Choices.tla` passes a
constant-level random argument to an action; getActions evaluates it once while
binding and every successor retains the resulting capture. The source-bound
action contexts, rather than a raw Next predicate evaluated in a new context,
are required to validate these different correlations. Native RNG draws need
not match TLC (ADR 0116), but valid words retain each source contract.

These primary controls are compact fixtures, not a refreshed conformance census
or release qualification. Regeneration uses the exact pinned jar, JRE and command
recorded by the normal oracle infrastructure; overwrite receipts only from actual
primary runs. Do not rewrite historical random-init captures.
