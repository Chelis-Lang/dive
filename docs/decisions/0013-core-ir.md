# 0013: Core IR (`dive-ir`)

- **Status:** accepted
- **Date:** 2026-10-08

## Context

`dive-ir` is the resolved, lowered form of a specification. Every backend consumes it:
- the evaluator and successor generator (`dive-eval`, ADR 0014);
- the checker, simulator and liveness checker, through the evaluator;
- the independent trace validator (`dive-trace`, ADR 0016);
- native compilation (`dive-compile`);
- a future exporter to Verus state machines compatible with verus-tla.

Its input is `dive-sema`'s semantic model (in progress on its branch). That model is SANY's semantic graph as an arena:
- `OpAppl` nodes name built-ins by SANY's names (`$SetEnumerate`, `$BoundedExists`, ...);
- instantiated definitions are wrapped in `SubstIn` nodes;
- `LET`, labels, `@` and proofs are kept;
- level-checking results are stored on every node.

Lowering is therefore a port of how TLC walks SANY's graph, and this ADR fixes what that port must preserve.

TLC paths below are relative to `tlatools/org.lamport.tlatools/src/` at tlaplus `94d0c50`. Probes are in `crates/dive-api/tests/fixtures/checker/tlc-probes/`.

### TLC behaviour the IR must be able to express

1. **Call-by-need.**
   - Arguments of user-defined operators are passed as cached `LazyValue`s (`SymbolNodeValueLookupProvider.java:124-146`).
   - Each zero-arity `LET` definition becomes a fresh `LazyValue` every time the `LET` is evaluated (`Tool.java:1849-1861`).
   - `INSTANCE` substitutions are bound lazily in the context (`Tool.java:1864-1873`).
   - A cached value is reused only under TLC's compatibility test: same tool, states that agree on every assigned variable, equivalent `EvalControl` (`LazyValue.getCachedValue`; `TLCState.java:246-276`; `EvalControl.java:92-110`).
   - Laziness is observable. An unused argument that would fail raises no error, and `Print`, `TLCSet` and `RandomElement` run once per lazy instance, not once per use.
2. **Constant definitions are evaluated once.**
   - Every zero-arity definition whose body is constant-level is evaluated at start-up and cached. A failure is swallowed, and the definition stays lazy (`SpecProcessor.java:341-381`).
   - `C <- Op` overrides evaluate `Op`'s body once (`:274-333`).
   - So `R == RandomElement(S)` at constant level is drawn once per run.
3. **Splitting the next-state relation into actions** (`Tool.getActions`, `Tool.java:269-419`).
   - `\/` splits.
   - `\E` over a domain that evaluates at constant level splits into one action per binding, in enumeration order. If the domain is empty or fails to evaluate, the whole `\E` is one action.
   - A user-defined operator whose arguments are all constant-level is inlined, and the actions inside it take its name.
   - `LET` and labels are transparent. A `SubstIn` with substitutions is not split.
   - Anything else is one action.
   - Actions are numbered after the initial predicates (`Tool.java:225-241`).
   - Each action's printed label is `<Name(params) line L, col C to line L2, col C2 of module M>` (`Action.java`; `tla2sany/st/Location.java:344-360`).
   - Probe `EvalErrorMidLevel` shows a split action labelled with its operator's name (`A`).
4. **Initial predicates.**
   - With `INIT`/`NEXT`, `Init` is one predicate and is not split (`SpecProcessor.java:954-998`).
   - With `SPECIFICATION`, the formula is walked through conjunctions, `SubstIn`, `LET` and operator applications. Level-≤1 conjuncts become initial predicates. `[][A]_v` supplies the next-state action `A`, unnamed (`SpecProcessor.java:1044-1390`).
   - Probes `StutterSpec` and `StutterInNext` confirm the consequence:
     - `SPECIFICATION Init /\ [][Next]_x` explores `Next`. It generates no stuttering steps and can deadlock.
     - `NEXT [Next]_x` explores `[Next]_x`. It generates and counts a stuttering successor for every state.
5. **Java overrides replace definitions.** TLC installs a Java implementation for many standard-module operators and ignores their TLA+ bodies (`SpecProcessor.processModuleOverrides`, `:2114-2324`). Some bodies are placeholders: CommunityModules `Json!ToJson` is `CHOOSE` over `STRING` (survey probe P3).

### Constraints from the other consumers

- **Verus export** (`docs/survey/adjacent.md`).
  - verus-tla models an action as a guard plus a deterministic transition, with nondeterminism lifted into an `Input` parameter.
  - So the IR must keep three things recoverable:
    - `Next` as a disjunction of named actions;
    - each action's existential parameters;
    - each action's per-variable primed assignments (`x' = e`, `x' \in S`, `UNCHANGED`), separate from its guards.
  - Stuttering must be explicit. `SF_`, `\EE` and `\AA` have no counterpart.
- **Compilation** (`dive-compile`). Generated Rust cannot do name lookup at run time, and closures are only cheap when the binding structure is static. The IR therefore needs:
  - resolved references and fixed slot layouts;
  - explicit laziness points;
  - explicit recursion;
  - effect annotations that say which subexpressions may be reordered, shared or evaluated early.
- **Layering** (`xtask/src/layering.rs`). `dive-ir` may depend only on `dive-syntax` and `dive-sema`. It cannot hold `dive-value` values or read `dive-config`.

## Decision

### 1. One arena per specification, immutable after lowering

`dive_ir::Ir` holds every lowered module of one specification: definitions, expressions, declarations, origins and analyses. It is immutable after `lower(&Analysis) -> Result<Ir, Vec<Diagnostic>>`, and it is `Send + Sync`. It is shared behind an `Arc`.

| Id | Indexes |
|---|---|
| `DefId` | operator, function, theorem and assumption definitions (named or not), including instantiated (`I!Op`), `LET`-local and `LAMBDA` definitions |
| `ExprId` | expression nodes |
| `VarId` | state variables of the root module, in the root context's insertion order; TLC's order (`varLoc`) is the permutation `Ir::tlc_var_order` (ADR 0015 §5) |
| `ConstId` | `CONSTANT` declarations of every module (TLC binds the configuration's constants by name in all of them) |
| `DeclId` | `CONSTANT` and `VARIABLE` declarations of every module |
| `Slot` | a position in a definition's frame (parameters, bound variables, `LET` locals, recursive function names) |
| `StdOp` | an operator that TLC overrides in Java (§5) |

Ids are dense `u32`s assigned by a deterministic traversal: modules in dependency order (each file's inner modules first), the symbols of each module's context in SANY's insertion order, then its assumptions and theorems; definitions reached only through uses get ids when first reached. Lowering the same input always gives the same ids. Ids are not stable across edits; §7 covers caching.

### 2. Expressions

Expression kinds are explicit Rust enum variants, not SANY's `$`-names. A spec SANY accepts lowers to no error nodes. (`Unbound` and `Unevaluable` below are not errors: they are places where TLC itself raises an evaluation error, kept so that dive raises the same one.)

| Group | Variants |
|---|---|
| Literals | `Bool`, `Num` (the numeral's text; the integer mode decides at binding time whether it is too big, `TLC_INTEGER_TOO_BIG` under `--int=tlc`), `Decimal` (TLC refuses it at start-up, `TLC_CANT_HANDLE_REAL_NUMBERS`), `Str` (unescaped as SANY does) |
| References | `Var { var, primed }` and `Const(ConstId)` for declarations no instance substitutes; `Local(LocalRef)`, a slot of the current frame or an enclosing one (`LocalRef { up, slot }`); `SubstRef { decl, primed }` for declarations some instance substitutes, looked up dynamically (§3); `Def(DefId)` (zero-arity use); `Unbound(name)` for a name TLC's context cannot reach (only through subexpression selectors; TLC raises `TLC_CONFIG_UNDEFINED_OR_NO_OPERATOR`) |
| Application | `Apply { callee, args }`, where `callee` is an `OpRef`: `Def(DefId)` (including `LAMBDA`s), `Param(LocalRef)` (an operator parameter), `Std(StdOp)`, `Builtin(BuiltinOp)` (a built-in passed as an argument), `Const(ConstId)` or `Subst(DeclId)` (a constant operator), or `Unbound`. Each argument is an expression with its laziness (`Pass`, §3) or an operator reference. The set and equality built-ins are `Builtin { op, args }` |
| Logic | `Junction { kind: And/Or, bulleted: bool, items }` (n-ary, item order kept), `Not`, `Implies`, `Equiv`, `If`, `Case { arms, other }` |
| Binders | `Quant { kind: Exists/Forall/Choose, bounds, body }`, where each bound is a single slot or a tuple pattern with an optional domain (unbounded forms are kept so that evaluation can raise TLC's error); `SetFilter`, `SetMap`, `FcnCons`, `RecFcn` (recursive function definitions `f[x \in S] == e`) |
| Data | `SetEnum`, `Tuple`, `Record`, `RecordSet`, `FcnSet`, `Cartesian`, `FcnApply`, `Except { base, updates }` with `ExceptAt` for `@`, plus the set operators as `Std` or language built-ins |
| Scoping | `Let { defs, body }`; `Subst { bindings, body }` for instance substitutions; `Label { name, body }` (transparent, kept for diagnostics) |
| Actions | `Prime(e)` for primed non-variables, `Unchanged(e)`, `StepBox { action, sub }` (`[A]_v`), `StepAngle { action, sub }` (`<A>_v`), `Enabled(e)`, `Cdot(a, b)` |
| Temporal | `Always`, `Eventually`, `LeadsTo`, `WhilePlus` (`-+->`), `Wf`, `Sf`, `TemporalQuant` (`\EE`/`\AA`) |
| Not evaluable | `Unevaluable` (an `ASSUME`/`PROVE` used as a theorem name's body, proof constructs): TLC's "cannot be evaluated" error |

**Every expression node carries:**
- `origin`: file, byte range, SANY's line and column of both ends, and the instantiated definition (`I!Op`) it was lowered under;
- `level`: SANY's level (constant, state, action, temporal), taken from `dive-sema`;
- `effects` (§6).

**Frames.**
- Each definition owns a frame layout: parameters, then bound variables, zero-arity `LET` definitions (thunks) and recursive function names, each with a fixed slot. Substitutions have no slots: they are dynamic (§3).
- Where a definition sits is its `DefScope`:
  - `TopLevel`: its own frame, no parent;
  - `Local { parent }`: a `LET` operator with parameters, with its own frame whose parent is the frame the `LET` was evaluated in;
  - `LetValue { parent, slot, pass }`: a zero-arity `LET` definition, a thunk in `slot` of the parent's frame, whose body is evaluated in that frame;
  - `Inline { parent }`: a `LAMBDA`, or a `LET` definition reached through a subexpression selector from outside its `LET` (`COp(1)!(2)!G(3)`), lowered at its single point of use with a frame whose parent is the frame of the use. TLC finds neither in its context and evaluates the body in the context of the use, so names the selector's generated `LAMBDA` binds are visible in it (tlaplus `test-model/suite/test206.tla`).
- A local reference is `LocalRef { up, slot }`: `up` frames out from the current one. A slot is rebound on each iteration of its binder, so an evaluator with mutable frames must snapshot a frame when a closure captures it (a thunk, a lazy function, a lazy set filter).
- Nothing is looked up by name after lowering, except substitutions (§3).
- Unnamed `ASSUME`s and `THEOREM`s are definitions too, so that every expression belongs to a frame. TLC processes the literals of theorem statements at start-up (`SpecProcessor.processConstants`), so they are lowered; an `ASSUME`/`PROVE` statement is `Unevaluable`.

### 3. Laziness, `INSTANCE` and recursion

- **Laziness points are explicit.** These are the places TLC creates a `LazyValue`:
  - user-operator arguments;
  - zero-arity `LET` definitions;
  - `Subst` bindings.

  The evaluator implements TLC's caching rule at those points (ADR 0014).
  - Lowering marks an argument or binding `Strict` only when it is pure and cannot fail: a literal, a bound variable, or a tuple or record of these. Every other point is `Lazy`. Arguments of built-ins and Java overrides are `Direct`: not laziness points.
  - Evaluating such a node eagerly is indistinguishable from TLC.
  - A state variable is *not* strict. It can be unassigned (in `Init`, or primed while successors are generated), and TLC's enumerator sees through a lazy argument to the variable it names (`SymbolNodeValueLookupProvider.getVar`): `F(v) == v \in S` used as `Init == F(x)` assigns `x`. Evaluating `x` eagerly would fail instead.
- **`INSTANCE` is not expanded by textual substitution.**
  - An instantiated definition `I!Op` is a `DefId` whose body is the original body under a `Subst` node binding the instance's substitutions. Priming a substituted variable (`x <- e`, then `x'`) lowers to `SubstRef { primed: true }`, meaning `e` evaluated in the primed context.
  - **Substitutions are dynamic, as in TLC.** `I!Op`'s body calls the instantiated module's *other* definitions (`Op2`), which SANY does not copy: their references to the module's constants and variables must see `I`'s substitutions. TLC gets this from its single dynamic context: `Subst` bindings are consed onto the context (`Tool.evalImplSubstInKind`, `Tool.java:1864-1873`), operator bodies are evaluated in the caller's context extended with their parameters (`getOpContext`), and a declaration is looked up in the context before its configured value (`SymbolNodeValueLookupProvider.lookup`). So every reference to a substituted declaration, wherever it is, is `SubstRef { decl, primed }` (or `OpRef::Subst(decl)` for a constant operator): the evaluator takes the innermost active binding of `decl`, and only if there is none falls back to the declaration's own meaning (a root variable or a configured constant). Under `ENABLED`, TLC stops that lookup at the `ENABLED` for variables (`Context.branch`, cut-off lookup). A declaration no instance substitutes keeps the direct `Var`/`Const`.
  - **Parameters, bound variables and `LET` values are lexical slots.** TLC looks them up dynamically too, but the two agree: a closure over a parameter could only see a different binding if it were applied inside a newer activation of the definition that binds it, which needs a recursive operator with an operator parameter, and SANY rejects those (a `RECURSIVE` declaration cannot declare operator parameters). The one case where TLC's dynamic lookup reaches names the lexical scope does not have is the selector case of `DefScope::Inline` above.
  - This keeps TLC's lazy, cached substitution semantics. It also avoids the blow-up of copying bodies per instance, and keeps the `ENABLED`-under-substitution cases fixed upstream in `8a23cb1dc` expressible.
  - The pass `inline_pure_substitutions` (WP-A4) replaces a binding by its expression when the expression is pure and cannot fail. That is semantics-preserving, and it lets the evaluator and compiler see through common refinement mappings.
- **Recursion.**
  - `RECURSIVE` operators are ordinary definitions marked `recursive`. The call graph may be cyclic only through them.
  - Recursive functions are `RecFcn` nodes.
  - Evaluation depth is bounded by the evaluator and reported as a resource limit, never a crash (ADR 0014).

### 4. Actions

**The action tree.** `dive_ir::actions::split(ir, root, name, frame, redirects) -> Split` mirrors `Tool.getActions` and `getActionsAppl` exactly. Its nodes:
- `Disj { children }`: `\/` and bulleted `\/` lists;
- `Exists { name, body }`: a bounded `\E` (`OPCODE_be`). Its domains are enumerated in the empty state; whether they evaluate is decided then, not by level, since a state-level domain that does not read a variable evaluates;
- `Inline { def, constant_args }`: a use of a user definition, which TLC inlines when every argument has constant level (`getLevel`, including operator arguments, whose level `Arg::Op` carries);
- `Scope { body }`: `LET`, labels and substitution-free `Subst`s, which are transparent;
- `Leaf { name }`: one action;
- `Throws`: TLC throws while splitting here;
- `Configured`: a constant or constant operator, which depends on the configuration.

A definition's subtree is shared (`Split::defs`), so recursive operators give a finite graph. The rules the implementation had to make explicit, each checked against TLC (below):
- **Names.** An action is named after the innermost operator inlined around it. An application that is not inlined is one action named after the operator applied (`(OpDefNode) opNode`), including a Java override.
- **`LET` definitions are not bound while splitting.** A `LET` value used as a disjunct is found by name and inlined, under its own name.
- **Exceptions.** An operator whose lookup is not a definition, and is not one itself, makes TLC's cast to `OpDefNode` throw `ClassCastException`: a parameter or bound variable bound to a value, a declaration, a theorem name, an operator parameter. So does an expression that is not boolean at all. The nearest enclosing `Inline` catches it, keeps the actions already added, and adds the whole application as one action. An enclosing `Exists` catches it, removes what its body added, and becomes one action. At the top it is TLC's error.
- **A `Subst` with substitutions is a leaf.** For an instantiated `SPECIFICATION`, TLC wraps the `A` of `[][A]_v` in a new substitution (`Specs.addSubsts`) whose location is the `INSTANCE`'s, and that wrapper is the one action.
- **Selected subexpressions** (`Op!1`, SANY's `$Nop`) are leaves (`Expr::Selected`).
- **Overrides.** The configuration's `<-` replacements of definitions come first in TLC's lookup, so `split` takes them as redirects: a use of `Op` inlines `Other`, under `Other`'s name.

**From tree to actions.** Expanding `ExistsSplit` needs values, so it happens when the model is built (§8). The result is the flat, ordered action list TLC uses. Each action has:
- a leaf expression;
- constant bindings for its split variables and inlined parameters;
- a name;
- an origin;
- a sequential id.

If a split domain fails to evaluate or is empty, the whole `\E` becomes one action, as in TLC.

**Leaf node kinds.** The successor generator interprets leaves with TLC's `getNextStates` recursion (ADR 0014). The IR guarantees that every node kind that recursion distinguishes has its own variant and keeps its source order:
- junction items;
- `x' = e`, `x' \in S`, `x' \subseteq S`;
- `UNCHANGED` of a variable, a tuple or a zero-arity definition;
- `[A]_v`, `<A>_v`, `IF`, `CASE`, `\E`, `\A`, `=>`;
- application of a function-valued lambda.

**Labels.** `dive_ir::actions::label` formats TLC's `<Name(v1,v2) location>` (`Action.getLocation`). The values are those of the named definition's parameters bound by inlining, joined with `,` and no space. The location is the leaf predicate's. An unnamed action prints as `Action`.

**The `ActionShape` view.** `dive_ir::actions::shape(ir, leaf)` returns an `ActionShape` for export and diagnostics:
- `name`;
- `params` (split variables and their domains);
- `guards` (state-level conjuncts);
- `updates`: a map from `VarId` to `Assign(e)`, `In(set)`, `Subseteq(set)` or `Unchanged`;
- `residual`: action-level conjuncts that are not single-variable updates.

An action that is not a conjunction of guards and updates, after inlining, is `Opaque(reason)`. Checking never uses this view; it is not on the path to any verdict. The Verus exporter will:
- expand `[A]_v` to `A \/ UNCHANGED v`;
- refuse opaque actions, `SF_`, `\EE` and `\AA` with `unsupported`.

### 5. Standard modules and Java overrides

**`StdOp`.** An operator defined in a standard module that TLC overrides in Java lowers to `Callee::Std(StdOp)`. `StdOp` is an enum in `dive-ir`, one variant per overridden operator in:
- Naturals, Integers, Sequences, FiniteSets, Bags, Strings;
- TLC, TLCExt, Randomization, Json;
- `_TLCTrace`, `_JsonTrace`, `_Possible`;
- the CommunityModules operators dive implements.

The TLA+ body is kept for documentation, but nothing evaluates it unless `dive-stdlib` declares the body equivalent to TLC's override and a conformance test proves it.

**Matching and refusal.**
- Matching is by module and definition, as TLC's own matching is. A user module's definition that has the same name is not a `StdOp`.
- Lowering reproduces both of TLC's installation phases (`SpecProcessor.processModuleOverrides`, `SpecProcessor.java:2114-2324`):
  1. for each loaded file-level module `M` with a class `tlc2.module.M`, every unannotated public static method is matched by name (after `TLARegistry.mapName`) against the definitions in `M`'s context, including extended ones, modules in dependency order, a later match overwriting an earlier one;
  2. then every annotated method (`@TLAPlusOperator`, `@Evaluation`, `@TLAPlusCallable`) of the index classes, `tlc2.module.TLCBuiltInOverrides` and then the CommunityModules' `tlc2.overrides.TLCOverrides`, names a module and a definition.

  The match is by module *name* (a user module called `Sequences` is overridden too), and the override is attached to the definition's body, so instantiated copies (`S!Len`) share it.
- **Last write wins matters.** `Naturals` and `Integers` both have classes, and `Integers EXTENDS Naturals`, so in a specification that loads `Integers` the shared operators are `Integers`' methods, otherwise `Naturals`'. They differ: only `Integers.Divide` checks `-2147483648 \div -1`. `StdOp` therefore has one variant per *method* (`NaturalsDiv`, `IntegersDiv`), and the variant a definition gets depends on the modules loaded.
- **The table is extracted, not transcribed.** `StdOp` (158 entries) was generated from the class files of the pinned `tla2tools.jar` and `CommunityModules-202610040242.jar`: access flags, parameter counts and annotation values (`docs/provenance.md`). Each variant records how TLC calls it (`StdKind`: value, evaluating, callable) and whether dive plans to implement it (`StdSupport`): `Core` (WP-C3's scope), `Extension` (CommunityModules, on demand), `Introspection` (reads checker state: `TLCExt!Trace`, `CounterExample`, `ToTrace`, `PickSuccessor`, `TLCDefer`, `_TLCState`, `_Counts`) or `Io` (files, processes, environment).
- Lowering records a `Refusal`, naming the operator, at every use of a non-`Core` override, outside the bodies of other overrides. An overridden operator that dive does not implement is refused as `unsupported` when the model is built, if it is reachable from any role (§8; `Ir::reachable_defs`). `dive-model` also refuses `Core` operators `dive-stdlib` has not implemented yet. An override is never evaluated through its placeholder body.

### 6. Effects analysis

Lowering computes, bottom-up over expressions and to a fixed point over definitions:

| Effect | Set by |
|---|---|
| `impure` | `Print`, `PrintT`, `TLCSet`, `RandomElement`, `Randomization`, `JavaTime`, `TLCGet` of a register or a live counter, CommunityModules I/O |
| `live_tlcget` | `TLCGet` of `"generated"`, `"distinct"`, `"queue"`, `"diameter"`, `"duration"`, `"stats"`, `"all"`, `"all:named"` or an integer or `"s:"` register |
| `may_fail` | conservatively, anything that can raise an evaluation error |
| `uses_enabled`, `temporal` | `ENABLED` and temporal operators |
| `open` | reading or applying a constant, which the configuration may bind to a definition with `<-`; `dive-model` completes the analysis once it knows the binding |

`TLCGet` with a key that is not a literal is `impure` and `live_tlcget`. An application of an operator parameter has the effects of every operator passed as an argument anywhere in the specification (a closed-world bound). A use of a `LET` value has the effects of its definition.

The analyses decide:
- strict versus lazy points (§3);
- substitution inlining (§3);
- whether the checker can run in parallel or must fall back to sequential mode (ADR 0015);
- what `dive-compile` may hoist or share.

### 7. Origins, stable identities and serialization

- **Origins.**
  - Every node's origin maps to file, line and column for diagnostics, using SANY's (`JavaCC`'s) column rules: one column per UTF-16 unit, tabs to the next multiple of 8, `\n`, `\r` and `\r\n` ending lines, end column inclusive.
  - It also gives the TLC-format location string used in counterexample labels and in the `-dumpTrace` JSON `location` objects: `line L, col C to line L2, col C2 of module M`, where `M` is the file-level module of the file, as SANY names locations also inside inner modules.
- **Structural hashes.**
  - Each definition has a hash of its lowered body that ignores origins and ids (definitions, declarations and overrides are hashed by name). It is 64-bit FNV-1a, whose output is fixed across platforms and Rust versions. `dive-compile` uses it for its build cache, and IR caching uses it for invalidation.
- **Serialization.**
  - The IR derives `serde` serialization behind a version constant `IR_SCHEMA`. The on-disk format and the cache itself are WP-A4; the corpus tests already check that a JSON round trip gives an equal `Ir`.
  - The on-disk cache is keyed by SHA-256 over: dive's version, `IR_SCHEMA`, the bytes of every loaded module, the search path and the lowering options.
  - A cache hit must give an `Ir` equal to a fresh lowering; a test asserts this for the corpus.
  - Caching is an optimisation. The ADR 0006 start-up target (≤ 10 ms for a trivial spec) is met without it.

### 8. Binding the configuration: a new crate, `dive-model`

The IR is value-free and configuration-free. Constants, overrides and roles are bound in a separate stage, `dive-model`, which ports TLC's `SpecProcessor`.

**Input:**
- `Arc<Ir>`;
- the parsed `.cfg` (`dive-config`);
- `dive-value`'s `Ctx` (integer mode, value order);
- the `dive-stdlib` table.

**Output:** a `Model` holding:
- **Constant bindings:**
  - each `ConstId` is bound to a `Value` or to a `DefId` (`C <- D`). Model values are interned in the cfg's creation order (`ModelConfig::model_values`);
  - `<-` overrides of operators, as a redirect table. The IR is not rewritten, so it stays cacheable independently of the cfg;
  - the precomputed constant definitions, under TLC's rules (Context item 2), with failures left lazy.
- **Roles:**
  - initial predicates and next-state actions (the expanded action list of §4);
  - invariants, implied initial predicates and implied actions;
  - temporal properties (refused in M5);
  - `CONSTRAINT` and `ACTION_CONSTRAINT`;
  - `VIEW`, `SYMMETRY`, `ALIAS` and `POSTCONDITION`;
  - the deadlock flag.
- **Errors:** every configuration and specification mismatch, with TLC's `EC` code and message (`TLC_CONFIG_*`, `TLC_CANT_HANDLE_*`, `TLC_INVARIANT_VIOLATED_LEVEL`, ...).

**Why a separate crate.** The binding stage needs `dive-config`, `dive-value`, `dive-eval` (to precompute constants and expand splits) and `dive-stdlib`. `dive-ir` may use none of them, and `dive-eval` may not use `dive-config`. The checker, the simulator, the liveness checker and the trace validator all need the same binding. Duplicating a `SpecProcessor` port in each would be a soundness hazard.

**Layering change** (accepted by the lead on 2026-10-08, open question 11; in `xtask/src/layering.rs`):
- add `("dive-model", &["dive-ir", "dive-value", "dive-config", "dive-eval", "dive-stdlib"])`;
- add `dive-model` to the allowed dependencies of:
  - `dive-check`, `dive-liveness` and `dive-sim`;
  - `dive-trace`. This is permitted by the trace-independence rule, which only excludes the checker, liveness checker, simulator and store; see ADR 0016;
  - `dive-compile`;
  - `dive-api`.

### 9. Deliberately out of scope for M5

- Type inference. A side table for optional type hints is reserved for the Verus exporter.
- Common-subexpression elimination.
- Constant folding beyond TLC's own precomputation.
- Inlining of user operators beyond §3 and §4.

Each of these can change evaluation order, error behaviour or side effects, so each needs its own ADR and a semantics-preservation argument.

## Consequences

- The evaluator, the trace validator and the compiler see one resolved form with explicit laziness, so call-by-need and TLC's error behaviour are implemented once, at known points.
- Because `INSTANCE` is kept as substitution frames, refinement-heavy specs (for example Examples `allocator`, `Paxos`) do not grow with the number of instances. The evaluator pays for substitution lookups instead, which `inline_pure_substitutions` reduces.
- The action tree reproduces TLC's action list. Generated-state counts and counterexample labels can therefore match: they depend on how `Next` is split.
- The Verus exporter can be built later from `ActionShape` without changing the IR. It refuses what it cannot express.
- `dive-model` is a new crate and a layering change. Until the owner approves it, the binding stage cannot be shared, so the M5 plan sequences it early (`docs/design/m5-plan.md`, WP-B1).
- The IR depends on `dive-sema`'s model shapes. A change there means a lowering change, and the corpus round-trip test (lower everything SANY accepts) catches it.
- Revisit:
  - when the Verus exporter starts (type hints);
  - when `dive-compile` starts (whether frames need more structure);
  - if profiling shows substitution frames dominate evaluation.

## Amendments

**2026-10-08, WP-A1 and WP-A2 (lowering and the action tree).** Implementing them over every corpus module showed five details of the original text to be wrong or incomplete. The sections above now say what was built; the evidence is:

1. **Substitutions are dynamic (§2, §3).** The original `SubstRef { slot, primed }` bound substitutions to slots of the instantiated definition's frame. That cannot work: `I!Op`'s body calls the instantiated module's other definitions, which SANY does not copy, and their references to the module's declarations must resolve to `I`'s substitutions. TLC resolves them through its dynamic context (`Tool.evalImplSubstInKind`, `getOpContext`, `SymbolNodeValueLookupProvider.lookup`). `SubstRef` and `OpRef::Subst` now name the declaration and are looked up dynamically; `Subst` bindings have no slots.
2. **Variables are not strict (§3).** The original list of strict shapes included variables. An unassigned variable fails, and TLC's enumerator relies on a lazy argument naming a variable to assign it (`getVar`), so a variable argument stays lazy.
3. **Selectors reach `LET` definitions out of scope (§2).** A subexpression selector such as `COp(1, 2)!(3)!++(4, 5)` (tlaplus `test-model/suite/test206.tla`, which SANY accepts) applies a `LET` operator outside its `LET`, inside the `LAMBDA` SANY generates. TLC finds the operator by node, not in its context, and evaluates its body in the context of the use, where the `LAMBDA` binds the names the body needs. Lowering it as a separate top-level definition left 31 unresolved references in the tlaplus tests; lowering it inline, like a `LAMBDA` (`DefScope::Inline`), leaves none.
4. **Overrides depend on which modules are loaded (§5).** `Naturals` and `Integers` both override the arithmetic operators, with different code, and TLC's last write wins. `StdOp` has one variant per Java method, and matching reproduces TLC's order.

5. **The action tree needs more node kinds than the four first listed (§4).** TLC's split depends on exceptions as well as values. An operator its lookup cannot cast to a definition throws; the nearest inlining or `\E` catches it, and the two treat the actions already added differently. `LET` definitions are not bound while splitting. Configuration overrides redirect the split. The tree therefore has `Scope`, `Throws` and `Configured` nodes, and `split` takes the overrides. It was checked against TLC's own `Tool.getActions()` on the 141 small-tier Examples models: 1,994 actions with every name, parameter and label in order. It was also checked on 44 repository models, including 7 fixtures written to pin each rule (`crates/dive-ir/acceptance/README.md`).

Smaller additions: `LocalRef { up, slot }` for closures over enclosing frames; `Decimal`, `WhilePlus`, `Unbound` and `Unevaluable` nodes; `OpRef::Builtin`, `Const` and `Subst` callees, with `LAMBDA` folded into `Def`; definitions for unnamed assumptions and theorems; the `open` effect; `DeclId`; `lower` returns `Result<Ir, LowerError>`, failing only on rejected input or an internal bug, with refusals recorded in the IR.

**2026-10-08, WP-B1 (the binding stage, `dive-model`).** Porting `SpecProcessor` and checking it against TLC's own binding of 721 models (`crates/dive-model/acceptance/README.md`) settled how §8 is built:

1. **Roles are expressions with TLC's context.** TLC keeps every role as an `Action`: an expression and the `Context` its walk built (inlined operators' arguments, split `\E` variables, instance substitutions). `dive-model` gives each role as a `Pred` (expression and `dive_eval::Scope`, the one scope type the binding stage and the successor generator share); `dive-eval` builds and evaluates scopes (`Evaluator::scope`, `scope_call`, `scope_subst` and the others, `eval_scoped`). With its default `enumerate` feature, `dive-model` also gives the initial predicates and the split next-state relation as the successor generator's actions, built with WP-C2's `predicate_in` and `actions_in`; the trace validator builds it without.
2. **TLC's level bound, not SANY's levels.** Which constant definitions TLC evaluates at start-up, which conjuncts of a `SPECIFICATION` are initial predicates and how a `PROPERTY` splits are decided by TLC's own bound (`getLevelBound`), computed after the configuration is applied. It differs from SANY's level: a constant given a value has level 0, a Java override its annotation's `minLevel` (`TLCGet` is 1), and any operator named like a root variable has level 1. `dive_model::level` ports it.
3. **Start-up values are TLC's tool objects.** A definition evaluated at start-up keeps its value for every later use (`Evaluator::precompute`); but `C <- D` and `Op <- D` bind `C` and `Op` to `D`'s *node*, so every use of them evaluates `D`'s body again, `Print` included. `dive-eval` now follows that. A definition whose whole body is a numeral or string is never evaluated at start-up (the literal's node already holds its value), and TLC's lookup of it gives the literal.
4. **Module-scoped values and overrides reach instance copies.** `C = [M] v` and `Op <- [M] D` set the tool object of `M`'s definition's *body*; TLC's lookup of an instantiated copy (`I!Op`) looks through its substitution wrapper to that body, so the copies see them too (Examples `NanoBlockchain`).
5. **The override set comes from the loaded modules.** The IR's `StdOp`s are matched against the modules the front end loaded. TLC's `-dumpTrace json` makes SANY load `_JsonTrace` (and `Integers`) without extending the root module; `dive_sema::Options::extra_root_extendees` reproduces that for oracle comparison (`dive_model::DUMP_TRACE_JSON_MODULES`). TLC as users run it, and so `--compat=tlc`, does not load them.
6. **The IR carries what the binding needs from the front end:** `Module::axioms` (`AXIOM`s are never checked), `Module::tlc_constants` (SANY's `getConstantDecls` order, in which TLC binds and processes constants) and `Ir::lexical_names` (the token images SANY's lexer interns, for the name feed). `IR_SCHEMA` is 3.
7. **A next-state relation that is a bare `TRUE`, `FALSE`, `BOOLEAN` or `STRING`** is one action named after the built-in (`ActionName::Builtin`), as `getActionsAppl` names it.
