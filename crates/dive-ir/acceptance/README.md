# Lowering every corpus module

`dive-ir` must lower every specification `dive-sema` accepts (M5 plan, WP-A1). Each
construct either lowers or is refused by name. This page records how that is checked
and the latest results.

## In CI

`tests/corpus.rs` lowers every `.tla` fixture in the repository that `dive-sema`
accepts:
- the SANY cases and SANY's error corpus (`crates/dive-sema/tests/`);
- the TLC probes (`crates/dive-check/tests/fixtures/`);
- the harness and configuration fixtures.

For each one it checks:
- `dive_ir::validate::check`: references resolve, every node is a tree node reachable from
  a frame, frames nest correctly, laziness points are marked, declarations are used
  correctly, and recursion goes only through `RECURSIVE` definitions;
- that every node's level is SANY's level for the node it came from;
- that no reference is left unresolved (`Expr::Unbound`);
- determinism, and a JSON round trip.

`tests/lower.rs` checks each IR property of ADR 0013 on small specifications.

## On the external corpora

The corpora and module libraries come from the harness cache (`dive-conformance oracle
fetch`, `corpus fetch`). The module path is the one `crates/dive-sema/acceptance/README.md`
describes: the `.tla` files of the CommunityModules jar (`unzip CommunityModules-*.jar
'*.tla'`), then the Apalache and TLAPS library directories.

```sh
cargo build --release -p dive-ir --example lower_corpus
target/release/examples/lower_corpus \
  --after "$CM_TLA_DIR" --after "$APALACHE_DIR" --after "$TLAPS_LIBRARY_DIR" \
  --list NAME.list --twice > NAME.jsonl
```

The runner:
- analyses each listed file;
- lowers it if SANY would accept it;
- runs the checks above;
- with `--twice`, lowers it again and compares, and round-trips it through JSON;
- catches panics and reports them as a verdict.

It prints one JSON line per file and a summary on stderr. The summary counts refusals in
definitions reachable from the root module's own definitions and assumptions, that is,
what the specification can actually use. `--pretty` dumps the IR instead.

## Results (2026-10-08)

| Corpus | Files | Lowered | Not lowered (SANY rejects) | Panics | Invariant violations | Unresolved references |
|---|---|---|---|---|---|---|
| tlaplus/Examples `6a7b26a` | 424 | 424 | 0 | 0 | 0 | 0 |
| tlaplus `test/**` + `test-model/**` `94d0c50` | 808 | 659 | 149 | 0 | 0 | 0 |
| CommunityModules `78b98ae` | 78 | 60 | 18 | 0 | 0 | 0 |

The lowered counts equal the modules SANY accepts in each corpus
(`crates/dive-sema/acceptance/README.md`). In every corpus, lowering is deterministic and
the JSON round trip is exact. The Examples lower to 76,418 definitions and 714,057
expression nodes in 3.6 s (release build, every dependency re-analysed and re-lowered for
each root, run twice).

**`Unevaluable` nodes** (7,441 in Examples) are all theorem statements of the form
`ASSUME ... PROVE ...`, mostly from the TLAPS library modules the proofs extend. TLC never
evaluates them either.

**Refusals.** Lowering refuses no language construct. It refuses only Java-overridden
operators dive does not implement (`StdSupport` other than `Core`). Since WP-C3
implemented the `Bags` overrides and the CommunityModules operators the corpus uses
(`SequencesExt`, `Functions`, `FiniteSetsExt!FoldSet`, `Bitwise`, `Graphs`,
`UndirectedGraphs`, `DyadicRationals`, `IOUtils!atoi`, ...), 19 of the 424 Examples
specifications can reach one (52 before). Uses reachable from the root module, by feature
(2026-10-08):

| Operator | Why refused | Uses | Specifications |
|---|---|---|---|
| `IOUtils!IOEnv` | I/O (environment) | 18 | 7 |
| `CSV!CSVWrite` | I/O (files) | 9 | 6 |
| `SVG!NodeOfRingNetwork` | CommunityModules, not implemented | 8 | 2 |
| `SVG!SVGElemToString` | CommunityModules, not implemented | 5 | 5 |
| `TLCExt!Trace` | reads TLC's checker state | 4 | 3 |
| `IOUtils!IOEnvExec` | I/O (process) | 3 | 2 |
| `SVG!PointOnLine` | CommunityModules, not implemented | 3 | 1 |
| `CSV!CSVRecords` | I/O (files) | 2 | 2 |
| `IOUtils!IOExec` | I/O (process) | 2 | 2 |
| `IOUtils!IOExecTemplate` | I/O (process) | 2 | 2 |
| `Json!JsonSerialize` | I/O (files) | 2 | 2 |
| `Json!ndJsonDeserialize` | I/O (files) | 1 | 1 |
| `Statistics!ChiSquare` | CommunityModules, not implemented | 1 | 1 |
| `SVG!NodesOfDirectedMultiGraph` | CommunityModules, not implemented | 1 | 1 |
| `VectorClocks!CausalOrder` | CommunityModules, not implemented | 1 | 1 |

Whether a refusal affects a *model* depends on whether a role of its configuration reaches
it. `dive-model` (WP-B1) decides that. Many of these uses sit in animation, trace-export
or proof-support definitions that no checked role uses.

In the tlaplus tests, the refused operators that can be reached also include:
- `TLCExt!CounterExample` (35 specifications), `TLCExt!ToTrace`, `TLCExt!TLCDefer`,
  `_Possible!_Counts` (checker state);
- `IOUtils!IODeserialize`, `CSV!CSVWriteRecord`, `Json!JsonDeserialize`,
  `Json!ndJsonSerialize` (I/O);
- `FiniteSetsExt!kSubset` (CommunityModules, not implemented).

Operators in dive's core that the evaluator does not implement yet (`Json!ToJson`,
`TLCExt!AssertError`, `TLC!JavaTime`, `Randomization!RandomSubsetSet`, ...) are not
refused by lowering; the evaluator refuses them by name where it evaluates them.

## The action split (WP-A2)

`dive_ir::actions::split` builds the tree TLC's `Tool.getActions` walks. Expanding it into
TLC's list of actions needs values, so `dive-model` (WP-B1) does that. The check is
against TLC's own list.

**The oracle.** `ActionDump.java` constructs TLC's `FastTool` from the pinned jar, as
`tlc2.TLC` does. It prints, for one model:
- the next-state relation;
- the configuration's `<-` overrides;
- `tool.getActions()`, in order. Each action has its name, its label (`getLocation()`,
  the string TLC prints in traces), its location, its bound parameters and its whole
  context.

It reimplements nothing. TLC's coverage report (`-coverage`) cannot replace it: the report
merges actions that share a location, so it loses their count and parameters.
`dump_actions.sh` runs one JVM per model, in the model's directory, with the harness
classpath. The pinned JRE has no compiler, so it needs a JDK 11+ `javac`.

```sh
python3 -I acceptance/models.py goldens <repository> <examples-checkout> > examples.tsv
python3 -I acceptance/models.py repo <repository> > repo.tsv
acceptance/dump_actions.sh <tla2tools.jar> <javac> <pinned java> <libraries> < examples.tsv \
  > tests/fixtures/actions/tlc-2026.10.06.014338/examples.jsonl
acceptance/dump_actions.sh ... < repo.tsv > tests/fixtures/actions/tlc-2026.10.06.014338/repo.jsonl
```

The dumps are generated, never edited.

**The comparison** (`tests/common/action_oracle.rs`). Without an evaluator, dive cannot
decide the value-dependent choices itself:
- how many bindings a split `\E` has, in what order, or whether its domain evaluates;
- whether an inlined operator's arguments evaluate;
- whether splitting throws.

So the comparison lets TLC's list make each choice. It succeeds only if some consistent
choice reproduces the list exactly: every action's name, its parameter names, its label
with TLC's parameter values, the order and the count. Each choice point only allows the
alternatives TLC's code has, so a disjunct, inlining, name or location that dive got wrong
cannot be matched. The next-state relation is rooted as TLC roots it:
- the named definition for `NEXT`;
- for `SPECIFICATION`, the `A` of the `[][A]_v` at TLC's location;
- for an instantiated `SPECIFICATION`, TLC's substitution wrapper. This is one action
  labelled with the `INSTANCE`'s location, and the check is that a substitution with
  bindings at that location reaches a `[][A]_v`.

The configuration's `<-` overrides of definitions are applied as redirects.

`tests/actions.rs` runs the comparison in CI on the repository's 48 models: the 7
split-rule fixtures in `tests/fixtures/actions/specs/`, the 19 TLC probes and the harness
fixtures. TLC rejects 4 of the harness fixtures, so 44 are compared.
`examples/action_oracle.rs` runs it on the Examples:

```sh
cargo run --release -p dive-ir --example action_oracle -- --corpus <examples-checkout> \
  --dump tests/fixtures/actions/tlc-2026.10.06.014338/examples.jsonl \
  --after "$CM_TLA_DIR" --after "$APALACHE_DIR" --after "$TLAPS_LIBRARY_DIR"
```

### Results (2026-10-08, `tlc-2026.10.06.014338`, Examples `6a7b26a`)

| Models | Exact match | Instantiated `SPECIFICATION` (one action, label checked) | Mismatch | Actions |
|---|---|---|---|---|
| 141 small-tier goldens (140 exhaustive, 1 simulation) | 135 | 6 | 0 | 1,994 |
| 44 repository models | 44 | 0 | 0 | |

How the 141 models give their next-state relation:
- 5 use `NEXT`;
- 120 use a `SPECIFICATION` whose `[][A]_v` is in the root module;
- 6 use an instantiated `SPECIFICATION`;
- 10 have none (constant-only models).

42 configurations have `<-` overrides. Two of them, `FiniteMonotonic/MCCRDT` and
`MCReplicatedLog`, replace `Next` itself. TLC inlines the replacement, and so does
dive's split with the redirect. The largest model has 93 actions.

The split-rule fixtures pin down the rules that the Examples exercise least:
- an operator whose lookup is not a definition throws in `getActionsAppl` (TLC casts it
  to `OpDefNode`);
- an inlined operator catches that, keeps the actions it already added, and adds the
  whole application (`SplitThrows`: `G(TRUE)` then `G`);
- a `\E` catches it and removes what its body added;
- an empty domain, or one that reads a variable, makes one action (`SplitDomains`);
- `LET` definitions are not bound while splitting, so a `LET` value used as a disjunct is
  inlined by name (`SplitLet`);
- `INSTANCE` substitutions stop the split (`SplitSubst`);
- `<-` overrides redirect it (`SplitOverride`).

## TLC's variable order (WP-A3)

`Ir::tlc_var_order` gives the root module's state variables in TLC's order: the `varLoc`
order that `TLCStateMut` stores, fingerprints and prints a state in. It comes from
`dive_sema::Model::variable_decls`, a port of SANY's `ModuleNode.getVariableDecls`: the
reverse of the iteration order of the module context's Java `Hashtable`
(`crates/dive-sema/src/hashtable.rs`; ADR 0015 §5 has the algorithm).

**The oracle.** `VarOrderDump.java` reads the order out of the pinned jar and
reimplements nothing. Given a cfg, it constructs TLC's `FastTool` as `tlc2.TLC` does and
prints `SpecProcessor.getVariablesNodes()`, checking that each variable's
`UniqueString.getVarLoc()` is its index. Without one, it runs SANY as `SpecProcessor`
does and prints the root module's `getVariableDecls()`. It also prints the number of
keys in the root context (`keys`), read by reflection, for diagnosis.
`dump_var_order.sh` runs one JVM per module, in the module's directory, from a TSV of
`id`, directory, module and optional cfg.

```sh
python3 -I acceptance/var_order_specs.py tests/fixtures/var-order/specs
# one line per spec: <name> TAB <dir> TAB <name>.tla TAB <name>.cfg
acceptance/dump_var_order.sh <tla2tools.jar> <javac> <pinned java> "" < specs.tsv \
  > tests/fixtures/var-order/tlc-2026.10.06.014338/specs.jsonl
# one line per .tla file, the id relative to the repository (or the corpus), no cfg
acceptance/dump_var_order.sh ... "$LIBS" < repo.tsv > tests/fixtures/var-order/tlc-2026.10.06.014338/repo.jsonl
acceptance/dump_var_order.sh ... "$LIBS" < examples.tsv > tests/fixtures/var-order/tlc-2026.10.06.014338/examples.jsonl
```

`$LIBS` is the rest of the harness classpath (the CommunityModules jars, the Apalache and
TLAPS library directories). `repo.jsonl` covers the `.tla` files under
`crates/dive-sema/tests`, `crates/dive-check/tests/fixtures`,
`crates/dive-conformance/tests/fixtures` (without `UsesCommunityModules`, as for the
action split), `crates/dive-config/tests/fixtures`, `crates/dive-ir/tests/fixtures/actions`
and `crates/dive-value/tests/fixtures/specs`. The dumps are generated, never edited.

`tests/var_order.rs` compares `specs.jsonl` and `repo.jsonl` in CI, through both
`Model::variable_decls` and `Ir::tlc_var_order`. `examples/var_order_oracle.rs` compares
the Examples:

```sh
cargo run --release -p dive-ir --example var_order_oracle -- --corpus <examples-checkout> \
  --dump tests/fixtures/var-order/tlc-2026.10.06.014338/examples.jsonl \
  --after "$CM_TLA_DIR" --after "$APALACHE_DIR" --after "$TLAPS_LIBRARY_DIR"
```

### Results (2026-10-08, `tlc-2026.10.06.014338`, Examples `6a7b26a`)

| Modules | Compared | Two or more variables | Match | Mismatch |
|---|---|---|---|---|
| Generated (`tests/fixtures/var-order/specs/`, through `FastTool`) | 29 | 26 | 29 | 0 |
| Repository fixtures SANY accepts | 234 | 3 | 234 | 0 |
| Examples, every `.tla` file | 424 | 303 (1,639 variables, at most 40) | 424 | 0 |

The generated specifications have 1, 2, 5, 11, 12, 13, 25 and 50 variables, each with no
standard module, with `Naturals`, and with five standard modules. They also include
names with equal Java hashes in both declaration orders (`Collide`, `CollideRev`),
variables inserted two `Hashtable` resizes apart (`Epochs`), an `EXTENDS` diamond
(`Extends`), and named, unnamed and `LOCAL` instances with an inner module (`Instances`).
`dive-store`'s `tests/tlc_state_fingerprints.rs` checks the state fingerprints that
follow from this order against TLC's `-dump dot` output for the same specifications.
