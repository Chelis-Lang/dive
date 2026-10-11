from pathlib import Path

root = Path('/Users/jeff/code/dive-pluscal-rewrite-publication-work')
path = root / 'docs/user/pluscal.md'
text = path.read_text()
old = '''General scoped renaming, sequential procedure modifiers, historical translator flags,
format-width, file rewriting/checksums and TLC mapping-file parity are unfinished
in this guide's source. Reference-produced but SANY-invalid text cannot become
an admitted public artifact. The lower-level source parser/raw translator is a
separate contract, not a replacement for enclosing-module admission.'''
new = '''Bounded in-memory translation-block editing is also available:

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
translator is a separate contract, not a replacement for enclosing-module admission.'''
assert text.count(old) == 1
text = text.replace(old, new)
old = '''Both return admitted in-memory artifacts. Inspect `module_source` and the
nonproof fields before using the result. File rewriting and checksum
interoperability are separate unfinished features.'''
new = '''Both return admitted in-memory translation artifacts. Inspect `module_source`
and the nonproof fields before using the result. These process/procedure examples
do not establish Full editing admission; the bounded editing contract above
reports its own supported scope.'''
assert text.count(old) == 1
path.write_text(text.replace(old, new))
path = root / 'docs/user/README.md'
text = path.read_text()
old = '| Translate algorithms | [PlusCal](pluscal.md) | Admitted C/P sequential and disjoint finite processes, bounded macros, recursive procedure stacks and process/procedure fairness |'
new = '| Translate or edit algorithms | [PlusCal](pluscal.md) | Admitted C/P translation and bounded in-memory Full/body-only editing; process/procedure translation has a wider scope than editing |'
assert text.count(old) == 1
path.write_text(text.replace(old, new))
path = root / 'docs/native-checkpoint.md'
text = path.read_text()
old = '''symmetry. It refuses liveness, runtime
names/effects/checker reads and evaluator-driven deferred state values. These restrictions apply in both profiles and are reported explicitly.'''
new = '''symmetry. Pure runtime string constructors are admitted with the explicit
encounter-order rules documented below; TLC mode uses an inline one-worker
fallback while canonical mode may use the pool. Liveness, random/effectful
execution, checker reads and evaluator-driven deferred state values remain
explicit refusals in this delivery. These restrictions apply in both profiles.'''
assert text.count(old) == 1
path.write_text(text.replace(old, new))
print('Updated current user manual and recovery summary without broad translator or disk-write claims')
