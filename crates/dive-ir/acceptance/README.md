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
operators outside dive's standard-library core (`StdSupport`). In Examples, 52 of the 424
specifications can reach at least one. Uses reachable from the root module, by feature:

| Operator | Why refused | Uses | Specifications |
|---|---|---|---|
| `SequencesExt!IsPrefix` | CommunityModules, not implemented yet | 88 | 8 |
| `Functions!FoldFunctionOnSet` | CommunityModules | 40 | 18 |
| `Functions!IsInjective` | CommunityModules | 28 | 12 |
| `IOUtils!IOEnv` | I/O (environment) | 18 | 7 |
| `CSV!CSVWrite` | I/O (files) | 9 | 6 |
| `Functions!FoldFunction` | CommunityModules | 7 | 5 |
| `FiniteSetsExt!FoldSet` | CommunityModules | 6 | 2 |
| `Functions!AntiFunction` | CommunityModules | 5 | 3 |
| `IOUtils!atoi` | CommunityModules | 5 | 2 |
| `SVG!SVGElemToString` | CommunityModules | 5 | 5 |
| `SVG!NodeOfRingNetwork` | CommunityModules | 8 | 2 |
| `Bitwise!Xor` | CommunityModules | 4 | 2 |
| `DyadicRationals!Reduce` | CommunityModules | 4 | 3 |
| `TLCExt!Trace` | reads TLC's checker state | 4 | 3 |
| `Graphs!AreConnectedIn` | CommunityModules | 3 | 3 |
| `IOUtils!IOEnvExec` | I/O (process) | 3 | 2 |
| `SequencesExt!FoldLeft` | CommunityModules | 3 | 3 |
| `SequencesExt!SetToSeq` | CommunityModules | 3 | 3 |
| `SVG!PointOnLine` | CommunityModules | 3 | 1 |
| `UndirectedGraphs!ConnectedComponents` | CommunityModules | 3 | 3 |
| `CSV!CSVRecords` | I/O (files) | 2 | 2 |
| `IOUtils!IOExec` | I/O (process) | 2 | 2 |
| `IOUtils!IOExecTemplate` | I/O (process) | 2 | 2 |
| `Json!JsonSerialize` | I/O (files) | 2 | 2 |
| `SequencesExt!SetToSeqs` | CommunityModules | 2 | 2 |
| `Json!ndJsonDeserialize` | I/O (files) | 1 | 1 |
| `SequencesExt!LongestCommonPrefix` | CommunityModules | 1 | 1 |
| `Statistics!ChiSquare` | CommunityModules | 1 | 1 |
| `SVG!NodesOfDirectedMultiGraph` | CommunityModules | 1 | 1 |
| `VectorClocks!CausalOrder` | CommunityModules | 1 | 1 |

Whether a refusal affects a *model* depends on whether a role of its configuration reaches
it. `dive-model` (WP-B1) decides that. Many of these uses sit in animation, trace-export
or proof-support definitions that no checked role uses.

In the tlaplus tests, the refused operators that can be reached also include:
- `TLCExt!CounterExample` (35 specifications), `TLCExt!ToTrace`, `TLCExt!TLCDefer`,
  `_Possible!_Counts` (checker state);
- `IOUtils!IODeserialize`, `CSV!CSVWriteRecord`, `Json!JsonDeserialize`,
  `Json!ndJsonSerialize` (I/O);
- `FiniteSetsExt!kSubset` (CommunityModules).
