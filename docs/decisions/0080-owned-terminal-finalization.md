# ADR 0080: Owned terminal graph finalization and ordered postconditions

Status: bounded prerequisite implemented; ordinary certification remains open (#228).

## Context

Pinned `ModelChecker.modelCheck` runs `runTLC`, then (only without an earlier
error) `LiveCheck.finalCheck`, then `Tool.checkPostCondition`. Its separate
zero-action branch skips both final checks and postconditions when Init is
empty. Direct method returns, observer process exits and plain CLI deciding
messages/exits are different evidence.

The unchanged 8820 native producer evaluated Post before the temporal EOF
callback. The minimal `Init == x=0; Next == x'=1; Live == <>(x=2); Post == x=x`
therefore returned postcondition 2405 rather than selecting the actual temporal
violation. Retain that source/binary observation and all failed observer epochs.

## Decision

Mint a private capture completion receipt only after actual graph/statistics
matching and bounded row/root/stutter/target validation. Phase-L consumes that
receipt, meters its written product/record/done/source authority using the
original shared quota, and records actual ordered final PEM progress.
Every OOS commit matches private source-id/fingerprint stamps and every node
has actual write coverage; retained concrete stamps are checked separately.

An opaque deferred-post token retains an exact validated bound model and private
admission identity. Cloning the token preserves that identity; separately
admitting even a same-core public Model does not. Only the
diagnostic capture route removes its original postconditions from a private
clone used for safety search. Actual mutable evaluator-core identity is checked
against the genuine completion before evaluating posts. The ordinary safety
engine retains its existing postcondition behavior.

The producer-owned temporal path evaluates retained posts only after all final
SCC selections passed. A deciding temporal candidate skips the normal success-path post call but
retains the separate `LiveWorker.printTrace` nondetermining post evaluation; it
must still pass independent fresh memory and serialized JSON replay. Cached-role
getter throws during rendering and partial request candidates with deferred posts
remain named-refused rather than certifying a selected violation. Direct bounded scalar/
Boolean posts and the recorded constant-role rejection are supported; all other
post syntax remains named-refused. Public token use does not establish caller
ordering, ordinary model proof or a certified queue/clock lifecycle.

Terminal classifications are additive evidence in the existing owned candidate
pipeline, not a new caller-certified facade. Final graph selection without a
candidate is not ordinary Holds. Existing public mandatory/optional APIs, default
routes, quotas and ordinary refusal markers remain intact.

## Consequences and remaining acceptance

The graph admission scan is bounded by retained entry limits; Phase-L metadata
and transition visits consume its cumulative graph meter. The direct-post policy
uses 50,000 visit/text units and depth 128, with no aggregate heap or exact
evaluator complexity claim. Ownership checks alone never authorize effects.

Complete ordinary admission still requires real asynchronous queue/clock/service
composition, deciding stop/error/count/output authority, Java after-error writes,
checkpoint effects and the no-action branch. #228 remains open beyond this
bounded composition. Retain all constructor/binding/method failures and complete
reference selections; no inferred postcondition callback is presented as observed.

## Evidence

- [Runnable user contract](../terminal-outcome.md).
- [Complete reference/development selection](../../conformance/probes/terminal-outcome/README.md).
- `ModelChecker.java:modelCheck`, `Worker.java:addElement/run/doNextCheckLiveness`,
  `Spec.java:getPostConditionSpecs`, `Tool.java:checkPostCondition` at the recorded
  oracle SHA.
