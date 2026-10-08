# Changelog

All notable changes to this project are documented here. The project will follow
[Semantic Versioning](https://semver.org/) once the library API is published.

## Unreleased

### Fixed
- Conformance harness (issue #35): goldens and `dev capture` no longer record TLC as
  perturbed by `-dumpTrace json`, which loads `Integers` and its Java overrides into
  every spec. The deciding run omits it; a counterexample's JSON document comes from a
  separate trace pass that must reproduce the clean counterexample exactly. Provenance
  gains `trace_command`. The flag audit is in the harness README. All 141 Examples TLC
  goldens were regenerated with byte-identical payloads.
- `dive-eval` and `dive-value`: under TLC's 32-bit integers, `\div` in a spec that loads
  `Naturals` but not `Integers` follows `Naturals.Divide` (`Int::div_naturals`):
  `(0 - 1) \div 2` is `0`. The recaptured fixtures `ErGeqNaturals`,
  `ErPlusNonIntNaturals` and `RvNatDiv` now agree with dive.

### Added
- `dive-eval` (M5, WP-C1): the interpreter over the IR (ADR 0014 and its amendment):
  - every expression form with TLC's evaluation order, laziness (thunks under
    `LazyValue`'s caching rule), lazy sets and lazy functions, `CHOOSE` in the run's
    value order, dynamic `INSTANCE` substitutions, recursion with a depth limit, and both
    integer modes;
  - TLC's error classes and message texts, including the wrapping of errors raised in
    Java overrides; anything not reproduced exactly is refused as `unsupported`;
  - the `Naturals`, `Integers`, `Sequences`, `FiniteSets`, `Strings` and basic `TLC`
    overrides, and TLC's assumption checking;
  - the pending owner questions as options with TLC's behaviour by default: exploration
    order (open question 9, D-010) and comparisons (open question 12, D-009);
  - 133 fixture specifications captured from the pinned TLC, plus `dive-value`'s 84,
    matched byte for byte; every `ASSUME` of the Examples corpus and the
    CommunityModules tests is either `TRUE` or refused by name.
- `dive-value`: a function remembers whether TLC holds it as a tuple literal
  (`Fcn::is_tuple_literal`), deferred sets can be lazy `\cup`, `\cap`, `\` and `UNION`,
  and `Set::as_deferred`, `components` and `raw_elements` expose what the evaluator needs.
- `dive-ir` (M5, WP-A1): the core IR and its lowering from `dive-sema` (ADR 0013):
  - every expression and operator form as an explicit variant, with SANY's level, an
    origin with SANY's line and column, and effects (`impure`, `live_tlcget`, `may_fail`,
    `uses_enabled`, `temporal`, `open`);
  - explicit laziness points (operator arguments, zero-arity `LET` definitions,
    substitutions), `INSTANCE` kept as dynamic substitutions, explicit recursion;
  - recognition of TLC's Java overrides (`StdOp`, extracted from the pinned jars) and a
    refusal, naming the operator, at every use of one dive does not implement;
  - deterministic ids, structural hashes, `serde` serialization (`IR_SCHEMA` 1), a
    debug dump and an invariant checker;
  - every module SANY accepts in Examples, the tlaplus tests and CommunityModules lowers
    (`crates/dive-ir/acceptance/README.md`).
- `dive-ir` action tree (M5, WP-A2): `actions::split` reproduces TLC's split of the
  next-state relation into actions (`Tool.getActions`), including its exception paths,
  configuration overrides and TLC's action labels; `actions::shape` gives the
  guards-and-updates view. On the 141 small-tier Examples models it reproduces TLC's own
  action list (names, parameters, labels, order and count) for every model.
- TLC's state-variable order (M5, WP-A3): `dive_sema::Model::variable_decls` and
  `constant_decls` reproduce SANY's `ModuleNode.getVariableDecls`/`getConstantDecls`,
  the reversed iteration order of the module context's Java `Hashtable`
  (`dive_sema::hashtable`). `dive_ir::Ir::tlc_var_order` records TLC's `varLoc` order
  (`IR_SCHEMA` 2). `dive_store::state::StateFingerprinter::tlc` computes TLC's own state
  fingerprint in that order, the scheme `--compat=tlc` uses; the default profile keeps
  dive's two-level fingerprint in IR order. The order matches the pinned TLC on 29
  generated specifications, 234 repository modules and all 424 Examples modules, and the
  fingerprints match TLC's `-dump dot` output on all 116 states of the generated
  specifications, from the specification text (ADR 0015 §5).
- `dive-model`, the configuration-binding stage (stub), and `dive-eval`'s `enumerate`
  feature (stub). `cargo xtask check-layering` rejects a standalone `dive-trace` build
  that enables the enumerator, and the workspace gate builds `dive-trace` on its own
  (ADRs 0013 and 0016, accepted).
- Cargo workspace with one crate per charter component, workspace lints,
  pinned toolchain, crate layering enforcement and lean CI.
- AI authorship protection: commit-msg hook, CI check and agent configuration.
- `dive-value` (ADR 0012):
  - 16-byte immutable, `Send + Sync` values with shared compound values and cached hashes;
  - arbitrary-precision integers (default) and TLC's 32-bit integers with its overflow
    errors (`IntMode`);
  - dive's canonical value order (default) and TLC's intern order (`ValueOrder`);
  - lazy sets (`a..b`, `SUBSET`, `[S -> T]`, `S \X T`, `[a : S]`, `Nat`, `Int`,
    `STRING`, `Seq`, evaluator-driven filters and maps) with membership without
    enumeration;
  - TLC's checked equality, comparison and membership, with TLC's error messages,
    distinguishing record literals from string-domain functions as TLC does;
  - TLC's value printer and pretty-printer, counterexample state format and JSON form;
  - TLC's FP64 fingerprints behind a `Fingerprinter` trait, verified in all 64 bits;
  - iterative drop, equality, ordering and fingerprints, and streaming of lazy sets,
    so deep nesting and huge intervals are safe;
  - differential fixtures from the pinned TLC, the red-team review's adversarial
    tests, property tests and benchmarks.
- `dive-store` in memory (ADR 0015, M5 work package D1):
  - a sharded 64-bit fingerprint set and an exact store of canonical state
    encodings, both with a level-synchronous resolve in which the earliest position
    of each new state wins, giving identical results at any thread count and
    schedule, without `unsafe`;
  - dive's two-level FP64 state fingerprint over cached value fingerprints;
  - per-level frontiers of encoded states, split into blocks for workers;
  - a 12-byte-per-state trace log of parents and successor ordinals for
    counterexample replay;
  - TLC's two collision-probability estimates, reproducing every figure in the
    conformance goldens and the "actual" figure from TLC's own fingerprints;
  - property tests under randomized schedules, and benchmarks.
- `dive-value`: a lossless value codec and a canonical exact-store key
  (`codec`), cached value fingerprints (`TlcFingerprint::fingerprint_cached`) and
  `Interner::resolve`.
- `dive-syntax`:
  - a TLA+ lexer and parser with a lossless rowan CST, including the proof language;
  - parse acceptance matching the pinned SANY parser;
  - stable `DIVE-S` diagnostic codes and error recovery;
  - typed AST views (ADR 0009).
- `dive-sema` (M4): SANY-equivalent semantic analysis (ADR 0011):
  - module resolution for `EXTENDS`/`INSTANCE`, nested modules and `LOCAL`, with
    SANY's search order, the embedded standard modules and the root-file fallback;
  - name resolution, arity, instances and substitutions, labels, subexpression
    selectors, recursion, and the proof language's scoping;
  - level checking ported from SANY;
  - stable `DIVE-E`/`DIVE-W` codes, each cross-referencing SANY's `ErrorCode`;
  - a resolved semantic model for `dive-ir` (`docs/sema-model.md`);
  - acceptance identical to the pinned SANY on every corpus module, SANY's error
    corpus, 164 minimal cases and 12,441 mutants checked
    (`crates/dive-sema/acceptance/`).
- `dive analyze FILE [--output human|json] [--library-path DIR]... [--module-path DIR]...`,
  with a versioned JSON report (`schema_version` 1), through `dive_api::analyze_file`.
- `dive parse FILE [--output human|json] [--cst]`, with a versioned JSON report
  (`schema_version` 1), through `dive_api::parse_file`.
- `dive-config`: the TLC `.cfg` reader. It covers:
  - every keyword the pinned TLC knows, including `_PERIODIC`, `_RL_REWARD` and `_POSSIBLE`;
  - TLC's constant value grammar and `<-`, `[Module]` and `A!B` forms;
  - typed `ModelConfig` with spans;
  - stable `DIVE-C` diagnostics, each with TLC's `EC` code, exit status and line.

  It matches TLC's own reader on 886 corpus cfg files and 99 mutants.
- `dive parse [FILE.tla] --config FILE.cfg` reports cfg diagnostics, and adds a `config`
  object to the JSON report (`dive_api::parse_config_file`).
- `dive-conformance run --impl dive` now runs dive's front end against the SANY
  goldens:
  - `dive analyze` against full SANY (default), and `dive parse` against SANY's
    parse-phase verdict of each file (`--sany-phase parse`), derived from the same
    goldens;
  - the module search path is derived from `oracles.toml` like the oracle classpath;
  - models stay `unsupported` until `dive check` exists;
  - `run` takes several `--corpus` into one report, and the summary breaks outcomes
    down per corpus;
  - the report records the dive binary (SHA-256, version), its module path, the
    profile and options.

  Results on 2026-10-08: all 1,309 runnable modules agree in both phases (Examples
  424, tlaplus tests 807, CommunityModules 78).
- Corpora `tlaplus-tests` (tlaplus `test/**` and `test-model/**`, 808 modules) and
  `community-modules` (78 modules, including `.smoke/`) have committed SANY goldens.
  `modules-only` corpora take `include` globs and `[[corpus.exclude]]` entries with
  reasons; excluded modules are reported as `out-of-scope`. One module that crashes
  SANY is excluded.
- The nightly `conformance` workflow builds dive and runs it on the modules of all
  three corpora, in both phases.
- `dive-syntax`: `tokens::tokenize_spec`, SANY's `SPEC`-state lexer as a public API.
- `dive-conformance`: the differential harness. It covers:
  - oracle and corpus fetching with SHA-256 verification;
  - TLC `-tool` stream parsing and verdict classification, cross-checked against exit codes;
  - SANY parse goldens;
  - golden generation with provenance (ADR 0003);
  - runs against goldens, with worker-invariance and manifest drift checks;
  - JSON reports and `docs/conformance.md`;
  - the nightly `conformance.yml` workflow;
  - isolated per-run copies of each spec directory, with `-seed 0`, `-fp 0` and `-Xss` pinned;
  - a `resource-limit` class for JVM out-of-memory and stack-overflow runs;
  - strict comparison for non-oracle implementations at every worker count;
  - an `--impl-option` hook (ADR 0010), where result-changing options yield `divergence-mode`;
  - oracle roles (`gating`, `reference`): v1.7.4 is reference, used only when named and never gating;
  - `conformance/oracle-changes.toml`, with a `known-upstream-change` outcome for cross-oracle differences that registered upstream changes explain.
- `conformance/oracles.toml`:
  - per-oracle classpath (v1.7.4 runs without CommunityModules);
  - pinned TLAPS-library and Apalache module libraries.
- `dive-stdlib::random` (M5 work package C5; D-002, D-003):
  - `java.util.Random` ported bit for bit (`JavaRandom`), with TLC's
    `RandomGenerator.nextPrime` and `-aril`;
  - TLC's seeding (`Streams`, `RandomMode::Tlc`): one start-up stream for constant
    definitions, `ASSUME`s and `Init`, and one stream per expanded state seeded with
    `fingerprint XOR seed`; TLC's simulation worker seeds;
  - dive's replayable seeding (`RandomMode::Dive`): counter-based streams keyed by
    seed, state fingerprint or trace number, and position, identical at any worker
    count;
  - `RandomElement`, `RandomSubset` and `RandomSetOfSubsets` over `dive-value`
    values, drawing exactly as TLC's Java overrides, with TLC's error messages;
  - `-seed` parsing as TLC's `Long.parseLong`, and a fresh reported seed;
  - TLC fixtures at six seeds; dive reproduces all 728 printed values, the error
    messages, and the `SpanTreeRandom` golden's state counts from `-seed 0`.
- `dive-value`: `Set::as_fcn_set`, `as_rcd_set` and `as_product`.
- `dive-conformance dev capture --seed N`, for fixtures with a seed other than 0.

### Fixed
- `dive-value`: Rust `==`, `Hash`, `Debug` and `cmp_with` on an unnormalized set
  whose elements TLC cannot compare no longer mark nested unnormalized sets as
  normalized (issue #32). These operations now run in a quiet mode that records no
  TLC-observable side effect, and an element list computed that way is not cached,
  so a later TLC operation still normalizes the nested sets as TLC does.
