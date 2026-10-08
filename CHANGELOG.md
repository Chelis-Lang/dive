# Changelog

All notable changes to this project are documented here. The project will follow
[Semantic Versioning](https://semver.org/) once the library API is published.

## Unreleased

### Added
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
