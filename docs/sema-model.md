# The semantic model (`dive_sema::model`)

`dive-sema` produces a resolved semantic model of a whole specification. `dive-ir`
lowers from it, and so will tools that need name resolution or levels, such as the
language server. This page is the contract between them. ADR 0011 explains why the model
looks the way it does.

## Shape

A `Model` is an arena. Every semantic object is a `Node`, addressed by a `NodeId`.

| Node kind | SANY class | What it is |
|---|---|---|
| `Module` | `ModuleNode` | A module, with its context (the symbols it defines or imports), its definitions, instances, assumptions, theorems and top-level items |
| `OpDecl` | `OpDeclNode` | `CONSTANT`, `VARIABLE`, or a `NEW` symbol of an `ASSUME` (`DeclKind`) |
| `FormalParam` | `FormalParamNode` | An operator parameter, or a bound identifier of a quantifier, `CHOOSE`, set constructor, function, `LAMBDA`, `TAKE` or `PICK` |
| `OpDef` | `OpDefNode` | A user definition, a built-in operator, the name `I` of `I == INSTANCE M`, or a numbered non-assertion proof step (`DefKind`) |
| `ThmOrAssumpDef` | `ThmOrAssumpDefNode` | A named theorem, assumption or assertion step |
| `OpAppl` | `OpApplNode` | An application. Built-in constructs are applications of built-ins named as in SANY (`$SetEnumerate`, `$BoundedForall`, `$FcnApply`, `$Except`, …; see `dive_sema::builtins::BUILT_INS`) |
| `OpArg` | `OpArgNode` | An operator passed as an argument or substituted (`F(+)`, `F(LAMBDA x : x)`, `WITH Op <- G`) |
| `LetIn`, `Label`, `At` | `LetInNode`, `LabelNode`, `AtNode` | `LET`, `lab :: e`, `@` in `EXCEPT` |
| `Numeral`, `Decimal`, `Str` | | Literals |
| `SubstIn`, `ApSubstIn` | `SubstInNode`, `APSubstInNode` | The body of an instantiated definition, under the instance's substitutions |
| `Instance` | `InstanceNode` | An `INSTANCE` with its explicit and implicit substitutions |
| `AssumeProve`, `NewSymb`, `Theorem`, `Assume`, `UseOrHide`, `DefStep`, `NonLeafProof`, `LeafProof` | the proof classes | Statements and proofs |

**Entry points:**
- `Model::modules()`: the file-level modules in analysis order, each after its
  dependencies;
- `Model::root()`: the root module;
- `Model::builtins()`: the built-in operators;
- `Model::context(module.ctx)`: name lookup in a module, using `Key::Sym` for symbols and
  `Key::Module` for inner modules.

**Resolution:**
- An `OpAppl`'s `operator` is the declaration, parameter or definition the name resolves
  to.
- A use of an instantiated definition `I!Op` resolves to an `OpDef` named `I!Op`, whose
  body is a `SubstIn` around the original body. The `SubstIn`'s substitutions replace the
  instantiated module's constants and variables.
- A recursive function definition's body is `$RecursiveFcnSpec`, and a non-recursive one
  is `$NonRecursiveFcnSpec`. Each has the bound identifiers, their ranges and tuple flags.

**Levels.** `Model::level(id)` gives the level after level checking: constant, state,
action or temporal. Every definition, declaration and expression reachable from a module
has one. `LevelData` also holds SANY's level parameters, constraints and Leibniz flags,
for any consumer that needs them.

## Guarantees when `Analysis::accepted()`

- Every module the root depends on is present. A module appears once, after the modules
  it extends or instantiates.
- Every operator application resolves. Applications of definitions, declarations and
  parameters have exactly the operator's arity, and an operator parameter receives an
  `OpArg` of the right arity. Built-in constructs follow SANY's shapes. These are not
  always the built-in's nominal arity: a `TAKE` step is `$Take` with bound identifiers and
  no operands.
- No `Error` node is reachable from a module. (`Error` placeholders exist only after
  rejected input.)
- Operands are present except one: the guard of a `CASE` `OTHER` arm, which is `None` in
  its `$Pair`.
- Every reachable expression and definition has been level-checked
  (`LevelData::checked > 0`). The exceptions are declarations and formal parameters,
  whose levels are fixed when they are created; as in SANY, a parameter that its body
  never uses is never visited.
- Names are not shadowed, because TLA+ forbids it and SANY enforces it. A
  `FormalParam`'s name is therefore unique in its scope.

When the analysis is rejected, the model holds whatever was built before analysis stopped.
It is useful for editor features but not for lowering.

## For the Verus export

The charter asks that the IR keep a later export to Verus state-machine definitions
(`verus-tla`) possible. The model supports that in four ways:

- **It records the information such an export needs.**
  - State variables are the `VARIABLE` declarations of the root module's context.
  - Constants are the `CONSTANT` declarations.
  - Levels separate state predicates (`Init`) from actions (`Next`) and temporal formulas
    (`Spec`, fairness).
  - `ENABLED`, priming, `UNCHANGED`, `[A]_v` and `WF`/`SF` stay explicit built-in
    applications. They are not desugared.
- **It makes no TLC-specific choices.** It has no evaluation order, no `CHOOSE`
  representative and no value representation. Those belong to `dive-eval` and
  `dive-value`.
- **It keeps source names and spans.** Generated definitions can therefore point back to
  the specification.
- **Instantiation stays visible** as `SubstIn`. An exporter can choose to inline it or to
  keep module structure.

## Stability

The public fields are documented and may change with `dive-ir` until the library API is
published (charter: semantic versioning after the first release). Fields that hold SANY's
internal bookkeeping (recursive-section numbers, cached declaration lists, iteration
counters) are crate-private.
