# 0132: Independently replayed post-Next temporal source errors

- **Status:** accepted (implementation merged in #679; advance consumer notice posted)
- **Date:** 2026-10-31

## Context

Issue #502's unchanged `MinChooseFair` and `MinChooseFairSafeNext` fixtures
both produce pinned TLC GENERAL1000 and physical exit255. In the first, Next
fails before a complete successor; in the second, Next completes with a self
successor. The later, actually reached fairness row fails in both. TLC's row
diagnostic suppresses ordinary Next diagnostic replay, retaining the first
primary. Fairness syntax or a matching CHOOSE message proves none of this.

Pinned source `94d0c5070852b37b801bb8998c436383ab7ee37b`, Worker.java's
post-Next handling, LiveCheck.java95–128, OrderOfSolution.java141–169 and
Liveness.java901–1009 establish the ordering. AEAction bins precede EAAction
bins; all action bins execute before state bins and tableau insertion. A known
bin can fail before any tableau work is reached. The existing state-only
full-MUST/cache certificate cannot prove that new path.

## Decision

Reserve native trace version5 for the closed `temporal-source-row` endpoint.
Outer operation reports remain schema3. Complete `states` form the original
finite source prefix; its last row is the failing source, not a partially
assigned successor, generated final candidate or lasso. Example shape:

```json
{"format":"dive-trace","version":5,"variables":["s","x"],"states":[{"action":null,"values":{"s":0,"x":0}}],"ending":{"kind":"error","ec":"GENERAL","endpoint":{"stage":"temporal-source-row","source":{"module":"M","start":100,"end":140,"instance":null}}}}
```

The endpoint carries only original row-error source context. `ec` is its
untrusted underlying error name. No scope, atom/action selector, cached truth,
primary-class override or diagnostic-suppression flag is transported. Unknown
fields/stages and version substitutions fail closed. Version5 cannot carry an
old ending; versions1/2/3/4 retain their meanings and cannot carry this form.

The producer observes the real terminal or completed source boundary, retaining
its actual original Next failure, admitted targets and statistics. A terminal
observation grants neither successful SourceRow closure nor complete/prefix
exploration. Separate private source ownership supplies a new bounded
certificate; public borrowed GraphCheck models cannot acquire it.

Independent validation first checks the original finite prefix and safety
obligations, then rebinds for terminal execution. It enumerates actual Next
actions and candidate checks in order. Only a concrete singleton self row is
covered: an admitted distinct target refuses. Complete emissions before a
later failure are replayed; ordinary deadlock precedes the row. Candidate
errors, incomplete assignments, unknown global newness and alias history retain
their earlier priority or explicit refusal.

An independent scoped lowering reconstructs one DNF solution and its exact
ordered action/state bins, including WF's Enabled and changed-first
Nonstuttering semantics. Pure state-only residual tableau work is an opaque
barrier after these bins. A first error reached before that barrier may be
proved; reaching the barrier refuses. This supplies no residual truth,
complete labels or old full-MUST/cache authority. Multiple solution orders and
residuals containing actions refuse.

The private terminal runtime executes both the row and its diagnostic again.
Only the bounded actual ToolRuntime/GENERAL lifecycle establishes the retained
GENERAL1000/physical255 contract. Other primary/row lifecycles remain named
refusals. Serialized source and underlying code must match the first actual
row error; neither report metadata nor a producer certificate supplies replay
truth. The producer publishes the finite word and verified class only after
fresh independent validation succeeds.

## Consequences

The first boundary covers pure closed scalar source values and a separately
proved finite integer-domain CHOOSE. It adds no backend, dependency, truth
cache, general set-history proof or new lasso rule. VIEW, symmetry, effects,
runtime names, unknown construction history and unproved parallel source
ordering retain explicit refusal. Direct2179/2182 state-row errors and all
earlier endpoints keep their existing meanings.

This first slice requires exactly one complete initial emission and a one-state
source word. Independent terminal replay enumerates Init to exhaustion and
checks that actual sole emission before executing Next. An unproved or quiet
first source event disables the producer's optional lane. Later rows or multiple
initial emissions cannot borrow a skipped predecessor's temporal-row authority;
they require a future extension that reconstructs every earlier row.

Interruption, resource and internal failures carry typed progress and no Valid
certificate, holding result or fabricated completion. Validation proves the
word and error lifecycle, not BFS counts; counters remain actual producer
receipts. Human/JSON reporting and conformance consume the same validated word
and require its GENERAL header and physical exit, not matching metadata alone.

Known consumers must receive advance notice before landing and implement this
distinct terminal meaning before advancing an immutable producer pin. They
must reject unknown versions and cannot relabel version5 as an earlier version.
The legacy infallible reader refuses the new ending; fallible fresh-session
entrypoints validate it. The owner authorized the
[version 5 migration notice](https://github.com/Chelis-Lang/oyster/issues/61#issuecomment-6545302842),
posted before #679 landed. The notice includes the wire shape and independent
replay obligations.
