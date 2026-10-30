# 0125: Ordered source-bound terminal replay

- **Status:** accepted
- **Date:** 2026-10-30

## Context

The ADR 0120 before-first-successor boundary cannot establish an error after a
complete candidate, nor TLC's secondary diagnostic wrapper. ADR 0123 preserves
partial Next bindings, but initially refuses preceding complete candidates.
The pinned TLC error fixtures include both cases: `RoActions` and `SplitThrows`
reach complete candidates first, and the `ErWrap*`/`ErNext*` streams distinguish
an original exception from the exception deciding diagnostic classification.
Initial incompleteness has no TLC behavior counterexample: TLC reports the
partial assignment in its initial-state error instead.

## Decision

Consume a shared pure ordered driver in independent Next-error and partial-state
replay. Reconstruct source/configuration, hosts, interner and evaluator caches
privately, as in ADR 0103. Neither descriptor nor checker metadata chooses an
AST node, action or evaluation context. The driver follows configured actions
and actual generator callbacks, then completeness, constraints, action
constraints, fingerprint demand, invariants and implied safety obligations.
Earlier errors/failures take priority. A source-bound complete prefix retains
all configured safety checks; public Behavior validation is unchanged.

Global visited/newness and cross-state alias history are not reconstructed.
An in-model candidate needing invariant/new-state facts refuses, except the
first actual Init emission, which is constructively new. Excluded candidates
receive their unconditional checks. Fingerprint demand admits only already
materialized stored representations through a cooperative, fallible traversal;
raw/symbolic representations refuse without forcing. The fingerprint is never
used to infer state equality, newness or BFS statistics.

The optional closed `next-ordered-terminal` endpoint carries original source,
reported code and deciding code. Replay proves the actual primary exception,
then executes TLC's per-action enumerate-before-check diagnostic sequence on
that same private terminal runtime. Typed evaluator-constructor provenance
separates EvalException from tool RuntimeException; raw value errors without
proved constructor provenance refuse. Typed integer division-by-zero transport
retains its actual module exception identity. Strings, error-code similarity
and matching wrapper metadata are not authority. The old
`next-before-successor` endpoint retains its old meaning. Complete words remain
native version 1; this is an optional closed endpoint-stage extension.

Partial words remain native version 3 with wire kind `partial-state`.
`init-partial-state` with actual initial-state EC2102 extends the same binding
contract; `next-partial-state` is unchanged. Initial words have no complete
prefix and logical length one. Both phases compare exact assigned values,
unassigned names and observed generating source. Producers retain actual Init
action identity. Initial partial evidence is optional report-schema-3
`error_witness`; formal `counterexample` stays null. Rendering never forces
assigned compounds after the deciding event: unencodable/unavailable evidence
is explicitly unconfirmed. The witness alone changes neither verified flags
nor primary classification/count/depth/exit.

All replay uses the fallible/controlled channel. Interruption, resource failure
or bug transports no fake completed validation report. Host refusals and the
actual composition capability boundary remain in force. Action-local diagnostic
candidate storage uses checked reservations and cooperative polls, without a
hard total-memory guarantee for ordinary Rust allocations or primitive bulk
operations. Borrowed models and caller caches cannot mint source proof.

## Consequences

The ten historical partial Next cases and the bounded 25 Next/generator wrapper
cases gain independently checked words in both profiles. Exact assertion2132
is canonicalized only with mandatory actual source replay. This does not repair
canonical unclassified pinned streams, prove BFS counts or TLC parity, or grant
other constraint/invariant/implied-action/VIEW wrappers an exemption. Unknown
constructor provenance, global alias/newness requirements and unproved demand
paths remain explicit refusals. Future sites must establish their own actual
ordered primary/diagnostic sequence within this contract.
