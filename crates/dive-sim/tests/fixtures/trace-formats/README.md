# TLC trace dump receipts

`*.tlc-dump.json` and `*.tlc-dump.tla` are what the pinned TLC
(`tlc-2026.10.06.014338`, with the pinned CommunityModules jars on the class
path) writes for the specification and configuration of the same name, run
from this directory with a relative output path:

```sh
java -cp tla2tools.jar:CommunityModules.jar:CommunityModules-deps.jar tlc2.TLC \
  -config Ord.cfg -simulate num=1 -depth 10 -seed 1 -metadir st \
  -dumpTrace json trace_0.json -noGenerateSpecTE Ord
```

`dive simulate --trace-dir DIR --trace-format json|tla` writes the same
layouts (D-042), tested in `crates/dive-api/tests/simulate_trace_dir.rs`.

| Spec | Ending | What it shows |
|---|---|---|
| `Ord` | deadlock (exit 11) | a model value, a function, variables declared out of alphabetical order |
| `One` | deadlock in the initial state | a one-state trace: `action` is empty |
| `Fmt` | invariant (exit 12), `ALIAS Al` | the `ALIAS` record replaces each state and its fields are the `vars` |
| `Un` | invariant, scalar `ALIAS` | an `ALIAS` that is not a record is ignored; unnamed disjuncts take the enclosing name and their own location |

Observed TLC behavior:

- A state record's fields follow TLC's internal name table, not the
  declaration order: `Fmt` without its `ALIAS` (`VARIABLES x, s, f`,
  `EXTENDS Sequences`) printed `f, s, x`, while `vars` listed `x, s, f`.
  dive writes the declaration order in both; JSON readers and TLA+ record
  equality do not depend on it.
- With an absolute output path, the `tla` module header holds the whole path
  without `.tla` (`---- MODULE /tmp/a/trace_0 ----`), so the file does not
  parse. dive's files are named `trace_<id>.tla` with module `trace_<id>`.
- Neither layout records how the trace ends; a deadlock and a violation look
  the same.

Fmt's and Un's traces are TLC's random choices. Fmt has one behavior, so dive
writes the same trace; under `--compat=tlc` the bytes are the same, and in
dive's profile records list their fields in canonical order. Un has two
successors at each step, and dive does not reproduce TLC's choices (D-003).
