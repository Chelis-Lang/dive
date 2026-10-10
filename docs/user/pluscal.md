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
simultaneous actuals, local initialization and an acyclic nested call graph.
Read the [single-procedure contract](../pluscal-procedures.md) and
[procedure-graph guide](../pluscal-procedure-graphs.md) for executable C/P and
Rust examples, adjacent call/return behavior, budgets and pending strict replay.

Recursive calls, process-local stacks, general renaming, historical translator flags,
format-width, file rewriting/checksums and TLC mapping-file parity are unfinished
in this guide's source. Reference-produced but SANY-invalid text cannot become
an admitted public artifact. The lower-level source parser/raw translator is a
separate contract, not a replacement for enclosing-module admission.

Rust callers use `TranslationRequest`, `translate_file` or `translate_source`;
`TranslationReport` provides `accepted()`, `exit_code()` and `to_json()`.
Detailed boundaries and evidence: [normalization](../pluscal-normalization.md),
[single process](../pluscal-processes.md), [multiple processes](../pluscal-multiple-processes.md)
and [fairness](../pluscal-fairness.md). Historical studies retain their original
selections; the current guides must not be read as claiming the full translator.
