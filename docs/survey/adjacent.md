# Adjacent work

Data gathered 2026-10-08 from `gh api repos/<owner>/<repo>`, from shallow clones
(verus-tla, stateright, tree-sitter-tlaplus), and from crates.io API responses.
Activity means the GitHub `pushed_at` timestamp plus the latest commit or release.

Other TLA+ implementations (tla2, tla-rs, tla-mc, tlago, tlaplusplus and others)
are covered in `competitive.md`.

No contradictions with the charter or ADRs 0004/0005 were found here. Charter
appendix item 8 (Stateright idle since 2025-07) is confirmed.

## Summary

| Project | URL | Licence | Latest activity | Role for `dive` |
|---|---|---|---|---|
| verus-tla | https://github.com/anvil-verifier/verus-tla | MIT | HEAD `e7eda3f24b60` 2026-10-06; 38 commits; created 2026-05-28; 26 stars | Target shape for the future Verus exporter |
| Stateright | https://github.com/stateright/stateright | MIT | `ab8c8be9341` 2025-07-27 ("version 0.31.0"); crates.io 0.31.0, 2025-07-27; idle | Embeddable-API reference (and anti-pattern for liveness) |
| Apalache | https://github.com/apalache-mc/apalache | Apache-2.0 | HEAD `8677897cd394` 2026-10-03; release v0.62.3, 2026-10-01 | Typing and corner-case semantics reference; not an oracle |
| Quint | https://github.com/quint-co/quint (`informalsystems/quint` redirects here) | Apache-2.0 | HEAD `23a4b811ff2a` 2026-09-28; release v0.33.0, 2026-09-28 | Typing/effects reference; Rust evaluator design; ITF traces |
| tree-sitter-tlaplus | https://github.com/tlaplus-community/tree-sitter-tlaplus | MIT | HEAD `add40814fda3` 2026-02-17; last release 1.5.0, 2024-10-20 (crates.io 1.5.0) | Concrete-grammar and corpus reference for `dive-syntax` |

## verus-tla (anvil-verifier/verus-tla)

**What it is.** It is a Verus crate, `verus_temporal_logic` 0.1.0, edition 2021,
toolchain `1.96.0` (`Cargo.toml`, `rust-toolchain.toml`). It embeds TLA as a
logic and was ported from Anvil's `temporal_logic` module (README). It depends
on `vstd` from Verus `main` by git, with no pinned revision. CI rebuilds Verus
`main` daily (`.github/workflows/ci.yml:16-49`). Size is 3543 lines:
`src/rules.rs` has 3076 lines of proved lemmas, `defs.rs` 178,
`state_machine.rs` 59, `action.rs` 54, and `mutex_example.rs` 170.

**Core definitions** (`src/defs.rs`, `src/action.rs`, `src/state_machine.rs`):

- `Execution<T> { nat_to_state: spec_fn(nat) -> T }`, an infinite behaviour
  (`defs.rs:8`), with `suffix`.
- `StatePred<T> = spec_fn(T) -> bool` and `ActionPred<T> = spec_fn(T, T) -> bool`
  (`defs.rs:28-30`).
- `TempPred<T>` wraps `spec_fn(Execution<T>) -> bool`. Connectives are `always`,
  `eventually`, `later`, `not`, `and`, `or`, `implies`, `leads_to`, `entails`,
  `valid`, `stable`, `tla_forall` and `tla_exists`. Lifting is done by
  `lift_state`, `lift_state_prime` and `lift_action` (`defs.rs:33-178`).
- `enabled(a) = |s| exists |s'| a(s, s')` (`defs.rs:139`).
  `weak_fairness(a) = always(lift_state(enabled(a))).leads_to(lift_action(a))`
  (`defs.rs:158`). **There is no strong fairness** (grep for `strong`/`sf`).
- `Action<State, Input, Output> { precondition: spec_fn(Input, State) -> bool, transition: spec_fn(Input, State) -> (State, Output) }`
  (`action.rs`). Each action is a **guard plus a deterministic transition
  function**, with nondeterminism lifted into an `Input` parameter. The struct
  provides `forward(input)`, `weak_fairness(input)` and a proved `wf1` rule.
- `StateMachine { init, actions: Set<Action>, step_to_action, action_input }`
  and `NetworkStateMachine { init, deliver }` (`state_machine.rs`).
- `mutex_example.rs` shows the intended encoding. The state is a typed Rust
  struct (`bool`, `Map<Tid, ThreadState>`), `next` is an explicit disjunction of
  `action.forward(param)` including `stutter()`, and liveness is proved from
  `wf1`.

**Relevance to `dive` (Verus export path, IR constraints):**

1. **Typed state.** Export needs a Verus type for every variable (`int`/`nat`,
   `bool`, `Seq`, `Set`, `Map`, structs and enums). TLA+ is untyped, so the IR
   must be able to carry inferred or annotated types without making them
   mandatory. Snowcat/Quint-style typing is the reference (below).
2. **Actions as guard + function.** The IR should keep `Next` as a disjunction
   of named actions with their existential parameters (to become `Input`), and
   keep each action's per-variable primed assignments (`x' = e`, `UNCHANGED`)
   separate from its guards. Flattening into a single relational predicate would
   block export. TLC's own conjunct analysis does the same split, so the IR can
   preserve it cheaply.
3. **Explicit stuttering.** `lift_action` has no `[A]_v` subscript, and the
   example adds a `stutter` action. The exporter must expand `[][Next]_vars`
   into `Next \/ UNCHANGED vars`.
4. **WF only.** `SF_v(A)` and temporal `\EE`/`\AA` have no counterpart today.
   The exporter must refuse them (`unsupported`).
5. **Version churn.** verus-tla tracks Verus `main`. The exporter should emit
   source text against a recorded verus-tla commit, not link `vstd` into `dive`.

## Stateright (stateright/stateright)

**What it is.** It is an actor-system model checker in Rust, version 0.31.0,
MIT (`Cargo.toml`). On crates.io it shows 14.8 M total downloads and 618 k
recent (API, 2026-10-08). The last commit was 2025-07-27, and the project has
been idle since.

**API shape** (`src/lib.rs`, `src/checker.rs`):

- `trait Model { type State; type Action; fn init_states(&self) -> Vec<State>; fn actions(&self, &State, &mut Vec<Action>); fn next_state(&self, &State, Action) -> Option<State>; fn properties(&self) -> Vec<Property<Self>>; fn within_boundary(&self, &State) -> bool; … }`
  (`lib.rs:158-253`).
- `Property::always`, `::eventually` and `::sometimes` take a
  `fn(&M, &M::State) -> bool` (`lib.rs:276-304`).
- `model.checker()` returns a `CheckerBuilder` with `.threads(n)`,
  `.symmetry()`/`.symmetry_fn(f)`, `.target_state_count`, `.target_max_depth`,
  `.timeout`, `.finish_when`, `.visitor`, `.serve(addr)` (web explorer), and
  `.spawn_bfs()`, `.spawn_dfs()`, `.spawn_on_demand()` and
  `.spawn_simulation(seed, chooser)` (`checker.rs:144-286`).
- `trait Checker` provides `join`, `state_count`, `unique_state_count`,
  `max_depth`, `discoveries`, `discovery(name)`, `report`,
  `assert_properties()`, `assert_no_discovery(name)` and
  `assert_discovery(name, actions)` (`checker.rs:298-534`). These `assert_*`
  helpers make the checker usable from `cargo test`.
- State identity is a 64-bit `ahash` with fixed seeds (`lib.rs:348-389`). It is
  fingerprint-only, with no exact mode and no collision bound reported.

**Soundness caveat.** The rustdoc of `Property::eventually` says it "only work[s]
correctly on acyclic paths" and that an unmet property closed by a cycle "will
be ignored -- a false negative" (`lib.rs:289-293`). There is a test named
`fixme_can_miss_counterexample_when_revisiting_a_state` (`checker.rs:676`).

**Relevance to `dive`:** it is a good reference for the ergonomics the charter
asks of `dive-api`. That means a builder, a spawn/join split, typed discoveries
returned as paths of (action, state), and `assert_*` helpers for `cargo test`.
It is also an explicit counter-example on soundness: `dive` must never ship a
liveness check with silent false negatives, and must always report its
fingerprint mode.

## Apalache (apalache-mc/apalache)

**What it is.** It is a symbolic (SMT, bounded) model checker for TLA+ and Quint,
written in Scala, Apache-2.0. Release v0.62.3 came out on 2026-10-01. The
charter places Apalache-style checking out of scope. Examples' 46 `symbolic`
models run it with `--length=5` (`corpora.md`).

**Relevant material (paths in the apalache repo at `8677897`):**

| Topic | Path |
|---|---|
| Type system "Snowcat" and annotations (`@type:`) | `docs/src/apalache/typechecker-snowcat.md`, `docs/src/apalache/types-and-annotations.md`, ADRs `docs/src/adr/001rfc-types.md`, `002adr-types.md`, `004adr-annotations.md` |
| Supported or unsupported TLA+ features, known issues | `docs/src/apalache/features.md`, `docs/src/apalache/known-issues.md`, `docs/src/apalache/antipatterns.md` |
| Engineer-oriented TLA+ language manual (per-operator semantics, corner cases) | `docs/src/lang/*.md` (booleans, integers, sets, functions, records, tuples, sequences, conditionals, control-and-nondeterminism, user-operators, modules, …) |
| Informal Trace Format (ITF, JSON traces) | `docs/src/adr/015adr-trace.md` |
| TLC config handling | `docs/src/apalache/tlc-config.md` |
| Extra modules used by Examples | `src/tla/Apalache.tla`, `Variants.tla`, `Option.tla` |
| Regression specs | `test/tla/` (292 entries) |

The ValidationTestSuite's `explanation.yaml` records 84 concrete TLC-vs-Apalache
divergences, for example CHOOSE over an empty filter (`corpora.md`).

**Relevance to `dive`:** use it as a typing reference for the optional type
layer the Verus exporter needs, as a catalogue of semantic corner cases (where
it differs from TLC, TLC wins per the charter), and for ITF as a candidate
interchange format for `--output json` counterexamples and `dive validate-trace`.
It is not an oracle.

## Quint (quint-co/quint)

**What it is.** Quint is a TLA-based specification language with its own syntax,
a type and effect system, a REPL, a simulator and tests. It is Apache-2.0, and
release v0.33.0 came out on 2026-09-28. The repository moved from
`informalsystems/quint` to `quint-co/quint` (the API redirect returns
`quint-co/quint`). Quint transpiles to TLA+ for TLC and drives Apalache for
`quint verify` (`docs/content/docs/model-checkers.mdx:7-25`).

**Rust evaluator.** `evaluator/` holds the crate `quint_evaluator` 0.2.0. It
compiles Quint IR to Rust closures (`evaluator/README.md`) and is used behind
`quint run --backend=rust`. Modules include `ir.rs`, `value.rs`, `evaluator.rs`,
`builtins.rs`, `nondet.rs`, `simulator.rs`, `itf.rs` and `storage.rs`. Its last
commit touching `evaluator/` was `ca573cf1ab` (2026-09-28). The README lists
parity gaps (`--seed`, `--mbt`, …) with the TypeScript simulator.

**Relevance to `dive`:** it gives a typing/effects reference (modes and effects
map closely onto TLA+ levels), a live example of a closure-compiling Rust
evaluator for TLA-like semantics (relevant to `dive-eval` and `dive-compile`),
and ITF trace emission in Rust. It is a reference only, since Quint's semantics
are its own.

## tree-sitter-tlaplus (tlaplus-community/tree-sitter-tlaplus)

**What it is.** It is a tree-sitter grammar for TLA+ and PlusCal, MIT, by Andrew
Helwer and Vasiliy Morkovkin (`Cargo.toml`). It ships as a Rust crate
(crates.io `tree-sitter-tlaplus` 1.5.0, 2024-10-20, 483 k downloads), as npm and
PyPI packages, and as WASM. In-repo version strings read `0.0.0`, because
releases set them. Examples CI uses it through `tree-sitter-tlaplus==1.5.0` for
feature detection (`.github/scripts/requirements.txt` in Examples). Users
include GitHub highlighting, nvim-treesitter, tla-web and tlauc (README).

**Stated correctness criterion** (README): valid TLA+ yields a correct tree.
Invalid TLA+ yields an "approximately correct" tree, permissively. It is "a
less-compelling choice as the backbone for an interpreter or model-checker".

**Structure:**

- `grammar.js` has 1629 lines. `src/scanner.c` is a 2039-line external scanner
  handling the context-sensitive parts. Its tokens are
  `leading/trailing_extramodular_text`, `_indent`/`_bullet`/`_dedent` (vertically
  aligned `/\` and `\/` junction lists), `_begin_proof`, `_begin_proof_step`
  (proof-level tracking), the keywords `PROOF BY OBVIOUS OMITTED QED WF_ SF_`,
  `_notify_pcal_algorithm_start/end`, `_double_excl` and `_error_sentinel`
  (`grammar.js:77-96`).
- **Node types:** `src/node-types.json` lists 273 named and 290 anonymous types.
  The supertypes are `_unit`, `_expr`, `_op`, `_proof`, `_number`,
  `_primitive_value_set` and `_number_set` (`grammar.js:100-108`). The named
  types cover modules (`module`, `extends`, `instance`, `substitution`,
  `local_definition`, `recursive_declaration`, `operator_definition`,
  `function_definition`, `module_definition`), expressions (`bound_infix_op`,
  `bound_prefix_op`, `bound_postfix_op`, `conj_list`/`conj_item`,
  `disj_list`/`disj_item`, `except`, `set_filter`, `set_map`, `choose`,
  `let_in`, `case`, `subexpression`, `step_expr_or_stutter`, `fairness`, …),
  proofs (`theorem`, `proof_step`, `qed_step`, `suffices_proof_step`,
  `use_or_hide`, `terminal_proof`, …) and 31 `pcal_*` nodes.
- **Operator precedence:** it uses tree-sitter named precedences `'0-0'` through
  `'17-17'`, following Specifying Systems p. 271 (`grammar.js:116-126`), with
  `prec.left` helpers (`:39-62`). TLA+ *ranges* are collapsed to single levels,
  and "Operator precedence range & associativity conflicts must be enforced on
  semantic level" (`grammar.js:995-996`). Ambiguities are resolved with GLR
  `conflicts` (`:128-…`) and `prec.dynamic` (for example `set_filter`,
  `:653-655`).
- **Unicode:** each symbol is a `choice` of its ASCII and Unicode spellings, for
  example `def_eq: choice('==', '≜')` and `land: choice('/\\', '\\land', '∧')`
  (`grammar.js:236-255`, `:847-1035`). There are 11 corpus files under
  `test/corpus/unicode/`.
- **PlusCal:** it is parsed inside the `(* --algorithm … *)` comment, triggered
  by scanner notifications, in both P- and C-syntax (`test/corpus/pluscal/`, 16
  files).
- **Corpus:** `test/corpus/` has 376 tests in 60 files. `test/run-corpus.sh`
  parses all of tlaplus/Examples (a submodule) and checks for no error nodes.
  tlaplus/tlaplus keeps a 312-test copy at `test/tla2sany/corpus/`, which its
  README says is "duplicated here" and may drift.

**Relevance to `dive-syntax`:** it is the best available node-type vocabulary
and test corpus for a lossless CST. Its external-scanner design for junction
lists and proof levels is a proven approach. But it is deliberately permissive,
so its acceptance is **not** an oracle. Precedence-range conflicts,
associativity errors and many malformed inputs must be rejected per SANY. Do not
depend on the C parser at runtime: it would bring `unsafe` and C into the
workspace, against the charter.

## Implications for dive

- Keep `dive-ir` export-friendly: preserve the named action disjuncts, their
  existential parameters, and per-variable primed assignments versus guards. Make
  stuttering explicit, and allow optional type annotations per variable. Record
  this as an ADR when `dive-ir` is designed.
- The Verus exporter must refuse `SF_`, `\EE` and `\AA` until verus-tla gains
  them, and must emit source against a recorded verus-tla commit.
- Copy Stateright's API ergonomics (builder, spawn/join, typed discoveries,
  `assert_*` helpers), but not its liveness shortcuts or its fingerprint-only
  state identity.
- Evaluate ITF (Apalache ADR 015, Quint `evaluator/src/itf.rs`) as the trace
  interchange format before freezing the `--output json` counterexample schema.
- Use Apalache's `docs/src/lang/` and ValidationTestSuite's `explanation.yaml` as
  checklists of corner cases to cover with minimal oracle-backed specs.
- Adopt tree-sitter-tlaplus's node-type names where they fit, and import its
  `test/corpus/` (MIT, with attribution) plus tlaplus's
  `test/tla2sany/corpus/` as parser round-trip tests. Use SANY, not tree-sitter,
  for accept/reject expectations. Handle precedence ranges in `dive-sema` or the
  parser exactly as SANY does.
