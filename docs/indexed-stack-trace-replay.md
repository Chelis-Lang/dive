# Replaying process-indexed tuple stacks

The independent validator can prove that every value of a finite state function
is a tuple, from Init and every Next branch. This allows strict replay of admitted
process-owned procedure stacks without trusting the frame values in a report.
It does not establish general runtime-name history or full PlusCal parity.

The runnable example is
[`IndexedStack.tla`](../conformance/probes/indexed-stack-replay/specs/IndexedStack.tla)
with its adjacent config. Owner 1 drops a tuple element, then owner 2 prepends 4.
The invariant `x<2` fails on the three-state word. Replay must check the original
Init, both actual indexed updates, every unchanged owner's values and the ending.

```sh
# A supported invariant violation exits 12; retain its report.
dive check conformance/probes/indexed-stack-replay/specs/IndexedStack.tla --compat=tlc --seed 0 --fp 0 --trace-format representation-v2 --output json > indexed-v2.json
dive validate-trace conformance/probes/indexed-stack-replay/specs/IndexedStack.tla --compat=tlc --seed 0 --fp 0 --trace indexed-v2.json --output json
# Legacy version 1 stays the default and is separately checked on this shape.
dive check conformance/probes/indexed-stack-replay/specs/IndexedStack.tla --compat=tlc --seed 0 --fp 0 --output json > indexed-v1.json
dive validate-trace conformance/probes/indexed-stack-replay/specs/IndexedStack.tla --compat=tlc --seed 0 --fp 0 --trace indexed-v1.json --output json
cargo run --locked -p dive-api --example indexed_stack_replay
```

The Cargo example checks both profiles and both transports, and independently
rejects an altered Next state. Version 2 can retain admitted stored representations;
transport success is not semantic proof. Use the same source, config, seed/fp,
profile and original field spellings. Do not rebuild token order from JSON.

## Source admission and limits

Init must establish a finite function constructor with independently tuple-proved
body; supported domains are closed finite sets or authoritative stored finite-set
bindings. Every Next disjunct and both conditional branches must preserve that
range. Single-index EXCEPT replaces only tuple-proved values. Fixed UNCHANGED
aliases retain ADR 0071 projection identity. Existential finite-owner actions use
analysis-only binder scopes; no owner values are sampled or enumerated to prove
shape. Indexed reads inherit only these private source facts. The actual selector,
record fields and relation still pass their existing bound-role/predicate checks.

Both tuple and range candidates share the existing 32,768-visit/depth 64 proof
policy, with at most 256 variables and 256 visits per operand proof. The separate
all-role audit remains limited to 8,192 visits/depth 64. These policies bound admission
work, not aggregate search memory or exact CPU time. Unsupported shapes or exhausted
work return the existing named runtime string encounter-order Unsupported.

Off-path string replacements, tuple/string branches, omitted assignments,
primed self-reference, nested EXCEPT paths, opaque maps, unproved domains and
runtime effects stay refused. A supplied tuple does not discharge an unproved
Init/Next branch. Validating a malformed word can fail at shape decoding or a
predicate, rather than yielding semantic replay credit. History construction
itself evaluates no expressions, forces no providers and interns no names.

Public arbitrary-state enabledness and lasso predicate APIs do not inherit this
whole-word induction. Canonical-profile validation stays distinct from TLC-order
validation. This increment neither changes the checker sequential fallback nor
widens Random/VIEW admission or mixed-set transport boundaries.

## Evidence boundaries

The immutable before census retains 40 selected procedure cases in both profiles:
32 sequential originals Valid and 32 altered words Invalid in each profile;
eight process originals/mutants Unsupported in TLC order, and eight originals
Valid/eight mutants Invalid in canonical order. Eight fresh clean primary process
runs all report the same invariant/count/depth observations as the native checker.
The first implementation census retains the exact same 40 cases and both profiles:
all 80 original words validate and all 80 altered words fail actual Next replay.
Every complete checker report (apart from `elapsed_ms`) and every emitted version-2
word is unchanged from the before census. A separate eight-process cross-product
checks both profiles, fingerprint/exact stores, widths 1/4/8, chaos seeds 0/7 and
both transports: 384 original words Valid and 384 semantic mutants Invalid.
Strict checks still report actual inline width 1; canonical checks report their
actual requested pool width. Another 80 process controls reject changed Init,
indexed updates, return-frame PCs, final values and missing endings. Labels that
name parameterized operators remain explicitly reported as not checked; these
results certify the actual Next relation rather than those labels.

The example's clean pinned TLC run and all four CLI variants agree on three
generated/distinct states, queue zero and depth three. The actual Cargo example
passes both profiles and transports. Two false-proof implementation mutations
fail the negative tests. The preserved pre-freeze source sweep found one stale
maintained-guide digest; its old envelope and corrected current-doc authority are
separate from all runtime producers. The final qualification uses a distinct frozen producer and remains separate
from these first implementation observations and the original ADR 0071 producer.
On 2026-10-12, its complete gate passed all 69 literal Python invocations
(64 distinct commands), all eight Rust stages and 1,698 tests with zero skipped.
The before-gate archive retains all 70 literal include sites and 65 targets
alongside the selected tracked source; this is not a hermetic toolchain or
dependency archive. The fresh release repeats the complete 80-case census,
384 width/store/transport cases and 80 frame mutants with the same outcomes,
plus eight clean primary pairs, the actual CLI/Cargo guide and all 37 conformance
models at widths 1/4/8. All 80 checker reports (except elapsed time) and words
remain equal to the first implementation. These are bounded selected proofs,
not general process-stack or namespace-history certification. See [ADR 0081](decisions/0081-indexed-stack-range-replay.md).
Issue #124 and F-029 remain partial until broader source-owned namespace histories
are proved; this is a bounded admission rule rather than family certification.
