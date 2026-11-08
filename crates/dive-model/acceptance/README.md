# Binding models as TLC does

`dive-model` ports TLC's `SpecProcessor` (M5 plan, WP-B1). This page records how it is
checked against TLC and the results.

## The oracle

`ModelDump.java` constructs TLC's `FastTool` from the pinned jar, as `tlc2.TLC` does, and
prints what TLC made of one model. It reimplements nothing. The output is one JSON line,
holding either TLC's error (code, class and message) or:
- the module table;
- the root constants' values and the definitions evaluated at start-up;
- the initial predicates and the actions, with the labels TLC prints in traces;
- the invariants, implied initial predicates, implied actions, temporal formulas and
  temporal properties, by the names TLC reports;
- the constraints, action constraints, view, alias, post conditions, symmetry and
  deadlock flag;
- `util.UniqueString`'s whole table, in token order (read by reflection);
- what TLC printed meanwhile.

`dump_models.sh` runs one JVM per model, from the model's directory, with the harness
classpath, and stops a model after `DUMP_TIMEOUT` seconds (TLC evaluates constant
definitions at start-up, and a few test models define sets too big to evaluate). With
`dumptrace`, TLC is set up as `-dumpTrace json <file>` sets it up. The pinned JRE is
Temurin 17 and has no compiler, so the script needs a JDK 17 or later `javac`; it
compiles with `javac --release 17`, because the pinned JRE cannot load class files
built for a newer release (a JDK 21 `javac` without `--release 17` fails with
`UnsupportedClassVersionError`).

```sh
python3 -I ../../dive-ir/acceptance/models.py goldens <repository> <examples-checkout> > examples.tsv
python3 -I test_models.py <tlaplus-checkout> > tests.tsv
python3 -I repo_models.py <repository> > repo.tsv
./dump_models.sh <tla2tools.jar> <javac> <pinned java> <libraries> [dumptrace] < X.tsv > X.jsonl
cargo run --release -p dive-model --example model_oracle -- --tsv X.tsv --dump X.jsonl \
  --after "$CM_TLA_DIR" --after "$APALACHE_DIR" --after "$TLAPS_LIBRARY_DIR" [--dump-trace]
```

`<libraries>` is the rest of the harness classpath (the CommunityModules jars, the Apalache
and TLAPS library directories). `test_models.py` pairs each tlaplus `test-model`
configuration with its module the way TLC's own tests do (the `ModelCheckerTestCase`
subclasses), or by name. `strip_dump.py` makes the committed copies.

**The comparison** (`tests/common/oracle.rs`) binds each model under `--compat=tlc` and
sorts it:
- *bound*: both bind, and every role agrees: the initial predicates and every action label
  in order, every role list by name, the view, alias and post conditions, the deadlock flag;
- *bound, refused*: as *bound*, but the model uses a feature dive does not check yet
  (liveness, `SYMMETRY`, an operator dive does not implement), which the checker refuses by
  name;
- *same TLC error*: both fail, with the same code;
- *rejected by both front ends*;
- *unsupported*: dive refuses to bind, naming the feature;
- *wrong*: anything else.

It also compares the name tables: whether every model value has TLC's token, and how many
of the names both tables hold are out of TLC's relative order (the complement of their
longest common subsequence).

In CI, `tests/roles.rs` runs the comparison on the repository's own 94 models (this crate's
fixtures, the action-split fixtures, the TLC probes and the harness fixtures), from the
committed dumps in `tests/fixtures/models/`, and `tests/startup.rs` compares the start-up
part of TLC's transcripts byte for byte: `Print` output of constant definitions, warnings,
configuration errors and assumption failures.

## Results (2026-10-08, `tlc-2026.10.06.014338`, Examples `6a7b26a`, tlaplus `94d0c50`)

| Models | Bound | Bound, refused | Same TLC error | Both front ends reject | Unsupported | TLC did not finish | Wrong |
|---|---|---|---|---|---|---|---|
| 141 small-tier Examples | 71 | 68 | 0 | 0 | 2 | 0 | **0** |
| 486 tlaplus `test-model` configurations | 247 | 168 | 15 | 22 | 32 | 2 | **0** |
| 94 repository models | 62 | 3 | 25 | 3 | 0 | 0 | **0** (+1 known) |

The same holds with TLC set up as `-dumpTrace json` sets it up and the front end loading
the same modules (`DUMP_TRACE_JSON_MODULES`): no model is wrong.

The known one is the harness fixture `Oom`: TLC runs out of memory evaluating `Inv` at
start-up, swallows the error (`processConstantDefns` catches `Throwable`) and keeps `Inv`
as an invariant; dive evaluates it to `TRUE`, and drops it with TLC's warning. The outcome
depends on TLC's heap, never an oracle fact. The two test models TLC does not finish
(`Github361`, `CyclicRedefineVars`) define sets TLC tries to evaluate at start-up for more
than two minutes.

**Unsupported, by feature.** Counted per refusal; a model can have several.

| Feature | Examples | Tests |
|---|---|---|
| temporal properties (liveness checking) | 55 | 161 |
| `SYMMETRY` | 11 | 25 |
| `_POSSIBLE`, `_RL_REWARD`, and `_Possible!_Counts` at start-up | 0 | 26 |
| a CommunityModules operator not implemented yet (`Bitwise!Xor`, `Functions!*`, `Graphs!AreConnectedIn`, `UndirectedGraphs!ConnectedComponents`, `DyadicRationals!Reduce`, `FiniteSetsExt!kSubset`) | 4 | 8 |
| an I/O operator (`IOUtils!*`, `CSV!*`), in a role, in the split or at start-up | 7 | 3 |
| `RandomElement`, `RandomSubset` (not wired into the evaluator yet), in the split or at start-up | 4 | 13 |
| another evaluator refusal at start-up (`Bags!SetToBag`, functions TLC keeps lazy) | 0 | 5 |
| a Java class next to a module (`Bits.class`, `UserModuleOverride.class`) | 1 | 1 |
| constant operators given values in the cfg (`OpRcdValue`) | 0 | 1 |
| a message dive cannot reproduce (a variable as a conjunct of `SPECIFICATION`; an unimplemented operator while evaluating a substituted definition) | 0 | 2 |

This table predates WP-C3. Since then, of the CommunityModules operators listed, only
`FiniteSetsExt!kSubset` is still refused; `Bags!SetToBag` is implemented; and
`RandomElement` and `RandomSubset` draw from the run's random stream when a checker
provides one (`BindOptions::host`), and are refused by name otherwise.

**Names.** Every model value has TLC's token: 382 of 382 in the Examples, 265 of 265 in the
tests, all in the repository models. No name both tables hold is out of TLC's relative
order: 0 of 74,945 in the Examples, 0 of 193,598 in the tests, 0 of 26,111 in the
repository models; with `-dumpTrace`, 0 of 90,902, 241,401 and 36,030. ADR 0012 ("For the
front end") records the measured order.

## TLC behaviour found on the way

Each has a fixture under `tests/fixtures/specs/` with TLC's transcript or binding.

1. **`C <- D` evaluates `D` again and again** (`SuPrintSubst`). TLC evaluates `D` at
   start-up once for every module whose constants include `C` (here twice: the module
   that declares `K`, and the root module that extends it), and again at every use of
   `C`, because `C` is bound to `D`'s node, not to its value. A `Print` in `D` prints at
   each. `Op <- D` behaves the same (`SuPrintRedirect`).
2. **A start-up evaluation that fails is retried in every module** (`SuPrintRetry`). A
   definition whose evaluation fails stays a definition, and is evaluated again for every
   module whose context holds it; a `Print` before the failure prints each time.
3. **A definition whose body is a literal is not a definition to TLC's lookup**
   (`RoLiteral`, `ErSpecValue`). `processConstants` gives the literal's node its value,
   and TLC's lookup finds it there: `Seven == 7` as a conjunct of the behavior
   specification fails with `TLC_CONFIG_OP_IS_EQUAL`, while `SPECIFICATION Spec` with
   `Spec == 7` makes `7` an initial predicate.
4. **A constant-level `INVARIANT` is dropped with a warning** (`SuInvConst`,
   `RoTrueRoles`), and one that is `FALSE` is a configuration error (`ErInvHasValue`),
   because TLC evaluates it at start-up.
5. **`TLCGet` is never evaluated at start-up** (`SuPrintLevel`): its override's
   `minLevel` is 1, so TLC's level bound makes any definition using it non-constant.
6. **Post conditions are looked up after checking** (`RoConstraints`): a `POSTCONDITION`
   naming a constant fails only once the state space has been explored.
7. **The invariant-level error's note depends on SANY's `levelParams`** (`ErInvLevel`):
   `Inv == x' = x` gets the note about a SANY bug, `Inv == x' = 1` does not.
8. **A next-state relation that is `TRUE` is an action named `TRUE`** (`RoActions`), as
   TLC's traces show (`<TRUE line ...>`).
9. **A `PROPERTY` quantified under an instance is not unrolled** (Examples
   `allocator/AllocatorImplementation`): TLC evaluates the domain of `\A c \in Clients`
   without the instance's substitutions, fails, and keeps the whole formula as one
   temporal property.
10. **`-dumpTrace json` changes the arithmetic** (`OvNaturalsGeq`): with it, `"a" >= 1` in a
    module that extends only `Naturals` reports `>=`; without it, `Naturals.GEQ`'s `>`.
11. **Module-scoped entries reach instance copies** (Examples `NanoBlockchain`): TLC
    puts the value of `C = [M] v` on the body of `M`'s definition, and finds it from
    `I!C` by looking through the instance's substitution wrapper.
12. **The configuration's sets are normalized when TLC processes constants**
    (`ErSetValue`, `AsTypedMv`): `{1, "a"}` and `{a_x, b_y}` fail as unexpected exceptions
    after the configuration has been bound.

**Through the successor generator.** WP-C2's corpus runner
(`crates/dive-eval/examples/next_corpus.rs`), which now binds every model with
`dive-model`, agrees with TLC's golden verdict, counts, depth and counterexample length
on 65 small-tier Examples models and on the counts of 44 more (liveness), with none
disagreeing; 26 are refused by name and 6 skipped (simulation or over the budget). It
found one binding fault the role comparison cannot see, now fixed: a module-scoped
`NoBlock = [Nano]NoBlockVal` must also reach `N!NoBlock` (Examples `NanoBlockchain`).

## Known gaps

- Absolute intern tokens are TLC's for model values only. Other names keep TLC's relative
  order, which is what ordering and printing depend on.
- `TLC_INVARIANT_VIOLATED_LEVEL`'s note follows the measured rule (a declaration read
  outside a prime), not SANY's `levelParams` computation itself.
- A definition TLC evaluates at start-up that dive's evaluator refuses is refused when its
  value decides the binding; otherwise it stays lazy, and the refusal surfaces if checking
  reaches it.
- TLC replaces the value of a definition evaluated more than once at start-up with the
  last; dive keeps the first. They differ only for definitions that draw random values.
- The parameterized-specification parameters of TLC's command line (`-dumpTrace`,
  `-loadTrace`) are modelled only as the modules they load; their post conditions,
  constraints and views are not.
