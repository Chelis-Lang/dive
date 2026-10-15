# Original action-target observations (#287)

`observations.json` is a compact identity receipt for the actual pinned primary
runs performed before this repair. Raw logs remain external and are not goldens.
The executable fixture is
`crates/dive-liveness/tests/fixtures/constructor-alias.tla`; its final form adds
the eliminated-action property to the earlier two modules.

The initial function is `[k \in {"a"} |-> 0]`; the generated record
`[a |-> 0]` has the same stored identity. Comparing that record with a tuple
inside `<>[][~(x' = <<0, 0>>)]_x` raises GENERAL (1000), physical exit 255.
The state-only `<>[]~(x = <<0, 0>>)` succeeds. Generating a function before
the record succeeds; reversing their order raises the action error. All
generated successors still count. The eliminated temporal action property
`<>(TRUE \/ <>[][~(x' = <<0, 0>>)]_x)` succeeds.

The residual state property `<>(x = <<0, 0>>)` raises a primary model error;
accepted native main instead refuses during independent validation. This
residual error-parity gap remains unresolved. No false violation is published.
The default native value profile succeeds for the initial action/state pair.
The rejected universal alias-state checking candidate is not used here.

TLC collection and action semantics are referenced in `docs/provenance.md`.
These observations establish no general cross-atom error ordering, residual
tableau implementation, Java queue/table layout or periodic lasso choice.

The owning `original_action_targets` test consumes the five expected cases;
API and CLI layers also retain explicit independently written case rows.
