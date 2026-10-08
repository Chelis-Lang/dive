# 0011: Semantic analysis, standard modules and `dive analyze`

- **Status:** accepted
- **Date:** 2026-10-08

## Context

M4 is `dive-sema`: SANY-equivalent semantic analysis. A specification must be accepted
or rejected exactly as the pinned SANY (tla2tools 1.8.0 at tlaplus `94d0c50`) does with
`-error-codes`. Without that flag SANY exits 0 on semantic errors
(`docs/survey/oracles.md`).

Several things had to be decided.

1. **Where the standard modules live.** SANY resolves `EXTENDS Naturals` to
   `tla2sany/StandardModules/Naturals.tla` inside `tla2tools.jar`. dive needs those
   sources at analysis time. `dive-sema` may depend only on `dive-syntax`
   (`xtask/src/layering.rs`), so it cannot use `dive-stdlib`, which sits above
   `dive-ir`.
2. **The search order.** SANY's resolver (`util/SimpleFilenameToStream.java`,
   `util/FileUtil.createNamedInputStream`) looks in this order:
   - the working directory;
   - the `TLA-Library` paths;
   - `tla2sany/StandardModules/` on the Java classpath;
   - the root of the Java classpath (where the CommunityModules jar keeps its `.tla`
     files, and where the harness puts the TLAPS and Apalache module directories);
   - a module of that name *inside the root file*, extracted line by line
     (`util/MonolithSpecExtractor`).

   The order matters. The TLAPS library ships its own `Bags.tla`, `FiniteSets.tla`
   and `RealTime.tla`, and the CommunityModules jar ships `Json.tla`. On the harness
   classpath the standard modules win; on a `TLA-Library` path the library would win.
3. **The structure of the analyzer.** Acceptance depends on hundreds of individual
   checks in SANY's `Generator`, its node classes and its level checker. Their
   interactions are subtle: shadowing is forbidden, a duplicate `VARIABLE` is only a
   warning, `[]` is checked against `$SquareAct` but not through `LET`, and so on.
4. **The command.** Semantic analysis could be `dive parse --semantic` or a new
   `dive analyze`.

## Decision

1. **Embed the standard modules in `dive-sema`** (`crates/dive-sema/stdlib/`,
   `include_str!`). They are the 20 `.tla` files of the pinned jar's
   `tla2sany/StandardModules/`, byte for byte, including the `_TLC*` trace modules,
   under tlaplus's MIT licence (`stdlib/LICENSE`, `stdlib/README.md`).
   - **Rejected: a new data crate.** A `dive-stdlib-tla` crate would add a layering edge
     and an ADR for 60 KB of text that only the analyzer reads.
   - **Rejected: reading them from `dive-stdlib`.** It is the wrong layer.

   `dive-stdlib` implements TLC's Java overrides of these modules; it reaches them
   through the semantic model, which records every module node as standard.
2. **Two kinds of library path, in SANY's order.** The search order is:
   1. the root file's directory;
   2. `--library-path` directories (SANY's `TLA-Library`);
   3. the embedded standard modules;
   4. `--module-path` directories (SANY's classpath root, where TLC finds
      CommunityModules);
   5. the module extracted from the root file's text.

   The conformance harness's setup is therefore `--module-path` for the CommunityModules,
   TLAPS and Apalache directories.
3. **Port SANY's semantics class by class, on the CST.** `dive-sema` walks
   `dive-syntax`'s tree directly. It ports these, keeping SANY's control flow so that a
   check can be compared line by line:
   - `Generator` (in `src/generate/`);
   - `SymbolTable`, `Context` and `SubstInNode`;
   - the `levelCheck` methods (in `src/level.rs`);
   - `SpecObj`/`ParseUnit` module resolution (in `src/loader.rs`);
   - the rejecting part of the PlusCal translation check (in `src/pcal_check.rs`).

   Where SANY aborts (it throws `AbortException`), the port stops analysing. Where SANY
   crashes after it has already reported an error, the port reports an error. Either way
   the verdict is the same.

   The semantic graph is an arena of nodes shaped like SANY's classes
   (`dive_sema::model`). It is the resolved model that `dive-ir` lowers from; its
   contract is `docs/sema-model.md`.
4. **Codes.** Diagnostics have stable `DIVE-Ennnn` (errors) and `DIVE-Wnnnn` (warnings)
   codes. Every diagnostic carries SANY's `ErrorCode` for the check, where SANY reports
   one, as `sany_code`.
   - SANY reuses 4003, 4004 and 4005 for unrelated checks. dive splits them into distinct
     codes and keeps SANY's number as the cross-reference.
   - Syntax errors keep their `DIVE-Snnnn` codes, including those in dependencies.
5. **`dive analyze FILE [--output human|json] [--library-path DIR]... [--module-path DIR]...`**
   is the command. It is a separate command, not `dive parse --semantic`, because:
   - its input is a specification (the root plus every module it imports), not one file;
   - it takes search paths;
   - it can fail for reasons a parse cannot, such as a missing module.

   `dive parse` stays a single-file syntax check, which is what an editor needs on every
   keystroke. `--output json` follows `dive parse`'s versioned conventions: the same
   diagnostic and span shapes, plus `file` indexes into a `files` table and `sany_code`.
   It has its own schema version, `ANALYZE_SCHEMA_VERSION`, set to 1.

## Consequences

- **Evidence.** Agreement with the pinned SANY on every corpus module, error-corpus
  specification and mutant checked is recorded in `crates/dive-sema/acceptance/README.md`.
  Each SANY behaviour that surprised us has a minimal spec with SANY's verdict in
  `crates/dive-sema/tests/sany-cases/`.
- **Re-pinning SANY.** Moving the pin means:
  - re-copying `stdlib/` from the new jar (and checking the diff);
  - re-running the acceptance procedure;
  - regenerating the verdict files.

  Level-checking and module-resolution fixes upstream (`docs/survey/oracle-history.md`)
  must be ported by hand. The class-by-class structure keeps that tractable.
- **The semantic model is SANY-shaped.** That is deliberate: `dive-ir` gets exactly the
  structure SANY gives TLC, including substitution wrappers for instantiated
  definitions and `$`-named built-ins. A cleaner IR is `dive-ir`'s job.
- **Not covered: PlusCal translation validation beyond the two rejecting outcomes.**
  The translator could, in principle, throw an unexpected exception while re-translating
  an algorithm whose comment and translation markers are both present. SANY would then
  reject. dive does not run the translator here; that is `dive-pluscal`'s job. No corpus
  module or mutant has shown it. The gap is listed in `docs/tlc-differences.md`.
