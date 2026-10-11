# Native PlusCal translation

```sh
dive translate docs/user/examples/Algorithm.tla --output json
```

Translation reads a C- or P-syntax algorithm in a module comment and returns an
in-memory artifact. The sample is a sequential P-syntax loop. Public CLI/API
acceptance requires the generated enclosing module to pass native syntax,
name and level analysis. `accepted: true` means an admitted artifact; it always
retains `model_proved: false` and `exhaustive: false`.

The JSON includes `module_source`, generated TLA/configuration text, actions,
process metadata, inserted labels, mappings, warnings and stage diagnostics.
The command does not rewrite the source. To save an admitted module, inspect the
result and explicitly write its `module_source`; then check it as a separate
model with an appropriate `.cfg`. Generated files are not a model proof.

Implemented paths include bounded sequential assignment/branches/loops/await,
supported automatic labels and omission rules, disjoint literal or finite
process blocks, process-local state and pinned fairness options. `--label` /
`--label-root ROOT`, `-nof`, `-wf`, `-sf` and `-wfNext` select their implemented
normalization/fairness settings. Resource limits and explicit name/scope
restrictions remain in both profiles.

Bounded macros now expand simultaneously before label/process normalization.
See [the macro guide](../pluscal-macros.md) for selectors, declaration order,
source origins, lexical/binder restrictions, budgets and checked C/P examples.

Disjoint explicitly labelled sequential procedures now support saved frames,
simultaneous actuals, local initialization, nested calls and recursive self/mutual
calls.
Read the [single-procedure contract](../pluscal-procedures.md) and
[procedure-graph guide](../pluscal-procedure-graphs.md) and
[recursive-procedure guide](../pluscal-recursive-procedures.md) for executable C/P and
Rust examples, adjacent call/return behavior, budgets and pending strict replay.

Disjoint finite processes may call shared procedures with per-ID stacks and
parameters, including recursive calls. Weak/strong process fairness and reachable
procedure label modifiers generate the pinned Spec, without claiming a liveness
proof. Read the [one-process guide](../pluscal-process-procedures.md),
[shared-procedure guide](../pluscal-multiple-procedures.md) and
[fair procedure guide](../pluscal-procedure-fairness.md) for executable C/P and
Rust examples, ownership, ordering, quotas and pending strict witness replay.

Bounded in-memory translation-block editing is also available:

```sh
dive translate docs/user/examples/Algorithm.tla --rewrite --compat=tlc --output json
```

For admitted sequential shapes, the default Full policy derives the algorithm
AST checksum and generated-body checksum. An explicit `--checksums tla` request
selects body-only editing and labels TLC mode `tlc+overrides`. The replacement
module is returned in JSON; the command does not write the source file. Read
[the editing contract](../pluscal-rewriting.md) and
[nested checksum expressions](../pluscal-checksum-expressions.md) for supported
shapes, stale-block handling, budgets and executable Rust examples. A matching
checksum alone is not an artifact or model proof; accepted artifacts also pass
native enclosing-module analysis.

General scoped renaming, sequential procedure modifiers, historical translator
flags, exact formatter output, filesystem writes and TLC mapping-file parity
remain unfinished. Full checksum derivation has a narrower scope than raw
translation: process/procedure/macro and unproved layout/name-fix shapes may
translate successfully while an editing request explicitly refuses. It never
silently falls back to body-only editing. Reference-produced but SANY-invalid
text cannot become an admitted public artifact. The lower-level source parser/raw
translator is a separate contract, not a replacement for enclosing-module admission.

Rust callers use `TranslationRequest`, `translate_file` or `translate_source`;
`TranslationReport` provides `accepted()`, `exit_code()` and `to_json()`.
Detailed boundaries and evidence: [normalization](../pluscal-normalization.md),
[single process](../pluscal-processes.md), [multiple processes](../pluscal-multiple-processes.md)
and [fairness](../pluscal-fairness.md). Historical studies retain their original
selections; the current guides must not be read as claiming the full translator.

Checked-in process examples can be translated directly:

```sh
dive translate docs/user/examples/MultipleCallExample.tla --compat=tlc --output json
dive translate docs/user/examples/FairCall.tla --compat=tlc --output json
```

Both return admitted in-memory translation artifacts. Inspect `module_source`
and the nonproof fields before using the result. These process/procedure examples
do not establish Full editing admission; the bounded editing contract above
reports its own supported scope.
