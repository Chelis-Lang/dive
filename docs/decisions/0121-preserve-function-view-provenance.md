# 0121: Preserve function receiver classes through evaluation and capture

- **Status:** accepted
- **Date:** 2026-10-29

## Context

ADR 0117's value design separates extensional identity from observable views
(`docs/developer/core/03-value-model.md` §8). A record literal and an equal
string-domain general function can have different TLC comparison behavior.
Report formatting cannot supply evaluation provenance.

Review of original-target candidate `5a2187ae` for #626 demonstrated false
success under TLC semantics: `Init == x = [a |-> 0]`, a next relation emitting
equal general-function/record aliases, and `P == <>[]~(x = <<0, 0>>)`.
The reference refuses the record/tuple atom comparison, while the candidate
reported success with folding enabled. Disabling folding restored refusal.
The nested case also failed. `Heap::intern_const` dropped literal flags;
startup `Value::Fcn` imports independently erased them.

A separate delayed-demand control still failed after correcting constants:
`Init == x = [a |-> 0] /\ n = 0`, `Next == n' = 1 /\ UNCHANGED x`,
and `P == <>[](n = 0 \/ ~(x = <<0, 0>>))`. Canonical frontier identities
discarded the source receiver class before the next state's atom was demanded.
The ordinary safety route's existing checked-comparison refusal remained
effective; this finding does not establish a separate safety false success.

Preserving a child view inside an unflagged parent also changes the parent's
identity invariant. Two representation-distinct general functions containing
extensionally equal record/general children must compare equal by value.
They cannot both enter the ordinary persistent identity class, whose equality
fast path treats unequal handles as unequal values.

## Decision

Preserve available tuple/record literal classes recursively through imports,
constant folding and representation-preserving interning. Keep canonical
`Heap::intern` behavior unchanged. Absence of a literal flag on a canonicalized
value does not prove general-function provenance; graph evaluation receives
original representations from the covered producer paths.

Reserve `flags::NESTED_VIEW = 16`. A representation-preserving parent containing
a view carries this transitive marker and uses the existing structural persistent
class. The marker does not assert that the parent is a tuple or record literal.
`Heap::eq` therefore performs structural value equality rather than an invalid
identity shortcut. Interning that view canonically still yields the ordinary
identity shared by extensionally equal values.

During graph capture, retain exact first-discovered current/next frontier rows
beside canonical rows. Evaluate sources from the original rows, including
`UNCHANGED` and `EXCEPT`; seen aliases never replace a discovered representative.
`Step.new`, predecessor identities and replay expectations remain canonical.
Runs without a graph sink allocate no additional frontier payload.

These rules preserve known function receiver classes only. Neither the new bit,
an interned handle, `Kind::Set` nor its normalization flag certifies raw set
construction, normalization demand or alias history. Unknown set-history routes
retain their existing explicit refusal boundary. No native all-vertex graph,
CSR integration, graph-limit increase or capacity claim follows.

## Consequences

Literal/general and nested aliases require behavioral TLC-comparison controls,
not only extensional `Value` equality. Test folded/unfolded values, imported
classes, memoization/hash-consing controls, seen originals and later source
demands in both profiles and sequential/parallel capture. Rejected candidates
and their source-bound measurements remain historical evidence.

Constants may retain additional structural views. Capture retains one extra
handle row per current/next frontier state, at eight bytes per variable, under
the existing capture admission and run-budget policies. Count their persistent
values even when speculative work is discarded. These are entry-count/lifetime
guarantees, not a total byte budget or measured memory benefit.
