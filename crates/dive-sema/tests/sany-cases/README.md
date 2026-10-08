# Minimal SANY semantic cases

Each `NAME.tla` here is a minimal module. `verdicts.txt` records the pinned SANY's
verdict on it: tla2tools 1.8.0 at tlaplus `94d0c50`, run with `-error-codes` by
`acceptance/run_sany.py`. `tests/sany_cases.rs` checks that dive agrees on every one.
`acceptance/make_cases.py` regenerates the files.

There are 164 cases: 61 accepted and 103 rejected by SANY. Most pin down an ordinary
check. The ones below show SANY behaviour that surprised
us when reading or porting SANY. Each was predicted from SANY's source, then confirmed on
the oracle.

## Accepted, with only a warning

SANY reports these as "Multiple declarations or definitions" warnings (code 4801,
`SymbolTable.addSymbol`). The two declarations have the same kind and arity. Only
definitions from the same parameter-free source count as "the same", so SANY warns,
returns success and keeps the first declaration.

| Case | Spec body | SANY |
|---|---|---|
| `DupVariable` | `VARIABLES x, x` | accept (warning) |
| `DupConstant` | `CONSTANT c` twice | accept (warning) |
| `DupTheoremName` | `THEOREM T == TRUE` and `THEOREM T == FALSE` | accept (warning) |
| `DupAssumeTheoremName` | `ASSUME A == TRUE` and `THEOREM A == TRUE` | accept (warning) |
| `DupStepNames` | two `<1>1.` steps in one proof | accept (warning) |
| `ExtendsTwoDefs` | `EXTENDS A2, B2`, both defining `x` differently | accept (warning 4800) |
| `UnnamedInstanceConflict` | `z == 2`, then `INSTANCE P` where `P` defines `z == 1` | accept (warning); the instantiated `z` is silently dropped |

Defining an *operator* twice is an error (`DupDefinition`, 4201), and so is mixing kinds
(`DupConstVar`, `DefThenTheoremSameName`, `ExtendsConflictKinds`).

## Level checking

- **`BoxLetAction` (accepted).** `[](LET y == 1 IN x' = x + y)` is accepted.
  `OpApplNode.levelCheck` rejects `[]A` for an action `A` only when `A` is an operator
  application other than `$SquareAct`. A `LET` (or a label) around the action hides it.
  The unwrapped forms `[](x' = x + 1)` and `[]A`, with `A == x' = x + 1`, are rejected
  with 4310 (`BoxAction`, `BoxDefAction`).
- **`ConstantModuleAnyLevel` (accepted).** A module with no variables can be instantiated
  with substitutions of any level, even a temporal one (`N <- []v`). `InstanceNode`
  checks substitution levels only for non-constant modules.
- **`TemporalInIf` (accepted).** `IF []TRUE THEN x' = x ELSE TRUE` mixes a temporal
  formula and an action. The mixed-level check (4313) covers `/\`, `\/`, `=>`, `<=>` and
  junction lists, but not `IF` or `CASE`. Compare `MixedTemporalAction`, which is
  rejected.
- **`UnusedBadLevelDef`, `UnusedBadLevelInLet` (rejected).** Level errors are found in
  definitions nobody uses, including definitions inside a `LET` that nobody uses.

## Modules

- **`LetInstanceInner`, `InstanceBeforeInner` (rejected, 4223).** Before an inner module
  is defined, SANY's module loader does not treat a reference to it as internal:
  - `INSTANCE Inner` inside a `LET`, or
  - a top-level `INSTANCE` written above the inner module.

  It then looks for a file `Inner.tla`. When it finds none, it falls back to extracting a
  module of that name from the root file's text (`MonolithSpecExtractor`). That finds the
  inner module itself, so the same module ends up loaded twice. Semantic analysis then
  reports "Multiply-defined module".
- **`ProofInstanceStep` (rejected, 4223).** The same applies to an `INSTANCE` proof step
  that names an inner module of the file.
- **`InnerRedefinesOuter` (rejected, 4201).** An inner module cannot define a name that
  the enclosing module has already defined, because it sees the enclosing module's
  symbols. `InnerSeesOuter` is accepted.

## Unicode

- **`UnicodeNat` (rejected, 4200).** `x == 1 \in ℕ` with `EXTENDS Naturals` is rejected
  with "Unknown operator: `ℕ`". The pinned SANY lexes `ℕ`, `ℤ` and `ℝ` as identifiers,
  but the standard modules define only `Nat`, `Int` and `Real`.

## Proofs

- **`TopImplicitStepLevel` (rejected, 4350).** In a theorem's top-level proof, `<*>a.`
  names level 0, and naming any step with `<*>` or `<+>` is an error. (The parser
  computes `<*>` as the enclosing proof level plus one, and the enclosing level is -1 at
  the top.)
- **`LabelOnAssumeProve`, `ProofNewInNested` (rejected, 4333).** Labels are allowed only
  inside a definition or a *named* theorem or step: SANY starts collecting labels only for
  named ones. A label inside an unnamed `THEOREM ASSUME ... PROVE ...` is therefore
  "not in definition or proof step".
- **`PickScope` (rejected), `PickAfter` (accepted).** The identifiers a `PICK` introduces
  are not in scope in the `PICK` step's own proof. They are in scope in the following
  steps.

## PlusCal translation check

SANY's parse phase also checks PlusCal algorithms against their translations
(`pcal.Validator`). Two of its outcomes reject the module:
- an unterminated string in the algorithm region;
- `END TRANSLATION` without `BEGIN TRANSLATION`, on which SANY crashes.

`src/pcal_check.rs` ports exactly those two outcomes. Its unit tests cover them.
