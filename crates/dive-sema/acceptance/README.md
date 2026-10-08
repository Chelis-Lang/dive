# Semantic acceptance against SANY

These scripts measure whether `dive-sema` accepts exactly the specifications that the
pinned SANY accepts with semantic analysis and level checking on. The pinned SANY is
tla2tools 1.8.0 at tlaplus `94d0c50`, run with `-error-codes`. Without that flag SANY
exits 0 on semantic errors (`docs/survey/oracles.md`).

The scripts produce a confusion matrix per corpus: both accept, both reject, dive-only
accept, SANY-only accept. They also regenerate the verdict files that
`tests/sany_cases.rs` checks.

They use only the Python 3 standard library. Run them with `python3 -I`, from a directory
that holds no downloaded files, and pass downloaded paths as arguments. The procedure
follows the syntax acceptance procedure in `crates/dive-syntax/acceptance/`, with
semantic analysis on.

## Inputs

**Oracle.** Use the harness's pinned artifacts (`conformance/oracles.toml`). `dive-conformance
oracle fetch` puts them in `$DIVE_CACHE_DIR`:
- `tla2tools.jar`;
- the Temurin 17 JRE;
- the two CommunityModules jars;
- the TLAPS library (`tlapm` `library/`);
- the Apalache modules (`src/tla`).

The **classpath** is, in this order: `tla2tools.jar`, `CommunityModules-*.jar`,
`CommunityModules-deps-*.jar`, the Apalache directory and the TLAPS library directory.
That is exactly the harness's SANY command (see any golden's `provenance.command`).

**dive's equivalent search path.** The root file's directory, the standard modules, then
`--module-path` for:
- a directory holding the `.tla` files at the root of the CommunityModules jar
  (`unzip CommunityModules-*.jar '*.tla'`);
- the Apalache directory;
- the TLAPS library directory.

On the classpath these come after `tla2sany/StandardModules`, so the standard modules
win over the TLAPS library's own `Bags`, `FiniteSets` and `RealTime`, and over the
CommunityModules `Json`. See ADR 0011.

**Corpora.** All are pinned in `conformance/corpora.toml`; `dive-conformance corpus fetch`
checks them out.
- tlaplus/Examples: all 424 `.tla` files.
- tlaplus/tlaplus: all of `test/**` and `test-model/**` (808 files, including the 126
  files of `test/tla2sany/semantic/error_corpus/`).
- CommunityModules: all 78 `.tla` files.

## Steps

1. **List files**, one absolute path per line, in `NAME.list`.
2. **SANY verdicts.**

   ```sh
   python3 -I run_sany.py "$JAVA" "$CLASSPATH" sany-NAME.jsonl NAME.list 2
   ```

   - Files are grouped by directory. SANY runs with that directory as its working
     directory and bare file names, as the harness does.
   - SANY stops at the first rejected file when given `-error-codes`, so a batch restarts
     after it.
   - Each file is classified from its own `****** SANY2` output segment:
     - **reject:** a parse error, a missing or circular module, `*** Errors:`, an abort,
       or a non-zero exit with no marker;
     - **accept:** otherwise. Warnings alone (`*** Warnings:`) do not reject.
3. **dive verdicts.**

   ```sh
   cargo run --release -p dive-sema --example acceptance -- \
     --after "$CM_TLA_DIR" --after "$APALACHE_DIR" --after "$TLAPS_LIBRARY_DIR" \
     --list NAME.list > dive-NAME.jsonl
   ```

   The example takes:
   - `--after DIR` for a directory searched after the standard modules (the CLI's
     `--module-path`);
   - `--lib DIR` for one searched before them (`--library-path`);
   - `--no-semantic` and `--no-level-check`, the equivalents of SANY's `-s` and `-l`,
     which are used for the per-stage matrices;
   - `--invariants`, which checks the model guarantees of `docs/sema-model.md` on every
     accepted analysis.
4. **Compare.**

   ```sh
   python3 -I compare.py sany-NAME.jsonl dive-NAME.jsonl [--codes]
   ```

   This prints the matrix and every disagreement. `--codes` also checks that, for each
   rejected `E<code>_..._Test.tla` file, a dive diagnostic cross-references SANY's
   `<code>`.

**Minimal cases.**
- `make_cases.py OUT` writes the specs of `tests/sany-cases/`.
- After step 2, `verdicts.py sany-cases.jsonl ../tests/sany-cases/verdicts.txt`
  regenerates their verdicts.
- The error corpus's verdicts (`tests/error-corpus/verdicts.txt`) come from the same
  run, restricted to `error_corpus/`.

**Mutants.** `mutate.py OUT SEED N FILE...` writes `N` semantic mutants per file. It:
- links each source directory's siblings into a mutant directory, so `EXTENDS` and
  `INSTANCE` still resolve;
- renames each mutant `<Stem>_m<i>`;
- applies one or two edits:
  - rename to an undefined name, or swap in another name of the file;
  - add or drop an argument;
  - prime an occurrence;
  - wrap an occurrence in `[]`, `<>`, `ENABLED` or `UNCHANGED`;
  - duplicate a definition or declaration;
  - add or remove `LOCAL`;
  - rename a bound identifier to a declared name;
  - drop a module from `EXTENDS`.

Sources are the corpus modules SANY accepts that are under 60 KB.

## Results (2026-10-08, pinned oracle)

**Full analysis.** Every corpus agrees with SANY on every module:

| Corpus | Files | Both accept | Both reject | dive-only accept | SANY-only accept |
|---|---|---|---|---|---|
| tlaplus/Examples `6a7b26a` (harness library path) | 424 | 424 | 0 | 0 | 0 |
| tlaplus `test/**` + `test-model/**` `94d0c50` | 808 | 659 | 149 | 0 | 0 |
| ↳ of which `error_corpus` (125 `.tla`) | 125 | 9 | 116 | 0 | 0 |
| CommunityModules `78b98ae` | 78 | 60 | 18 | 0 | 0 |
| `tests/sany-cases` | 164 | 61 | 103 | 0 | 0 |
| Mutants, wave 1 (all edits; seed 101, 4 per file, 1,130 sources) | 4,482 | 1,042 | 3,440 | 0 | 0 |
| Mutants, wave 2 (all edits; seed 202, 4 per file, 1,158 sources incl. TLAPS and Apalache libraries) | 4,558 | 997 | 3,561 | 0 | 0 |
| Mutants, wave 3 (`--ops prime,temporal`; seed 303, 3 per file, 1,158 sources) | 3,401 | 741 | 2,660 | 0 | 0 |

Across the 12,441 mutants:
- SANY rejected 9,661: 3,172 in its parse phase and 6,489 in semantic analysis or level
  checking.
- dive's first diagnostics covered more than 30 distinct codes.
- The level-focused wave 3 alone produced 530 level-argument errors (`DIVE-E0600`), 89
  mixed action/temporal errors, 81 non-constant `ASSUME`s, and every other temporal
  level check.

On the Examples corpus, the SANY verdicts above agree with the harness goldens under
`conformance/golden/tlc-2026.10.06.014338/examples/sany/`. All 424 are `accept`.

**SANY's error codes.** Each of the 114 rejecting `E<code>_..._Test.tla` files of the
error corpus has a dive diagnostic that cross-references the expected SANY code. The
other two rejected files are helper modules of the circularity tests. The 9 accepted
files are the `W48xx` warning tests and their helpers.

**Model guarantees.** `--invariants` found no violation on any accepted corpus module or
wave-1 or wave-2 mutant.

**Per stage.** dive with only the stage's phases enabled, against full SANY. This shows
what each stage catches; only the last row of each corpus is a claim of agreement.

| Corpus | Stage | Both accept | Both reject | dive-only accept | SANY-only accept |
|---|---|---|---|---|---|
| tlaplus tests | 1 module resolution (`--no-semantic`) | 659 | 22 | 127 | 0 |
| tlaplus tests | 2 + 4 names, arity, instances, proofs (`--no-level-check`) | 659 | 110 | 39 | 0 |
| tlaplus tests | 3 level checking (full) | 659 | 149 | 0 | 0 |
| sany-cases | 1 | 21 | 1 | 48 | 0 |
| sany-cases | 2 + 4 | 21 | 36 | 13 | 0 |
| sany-cases | 3 | 21 | 49 | 0 | 0 |
| Mutants wave 1 | 1 | 1,042 | 844 | 2,596 | 0 |
| Mutants wave 1 | 2 + 4 | 1,042 | 3,154 | 286 | 0 |
| Mutants wave 1 | 3 | 1,042 | 3,440 | 0 | 0 |

The sany-cases rows were measured on the first 70 cases, before the other 94 were
added. The Examples and CommunityModules rows are identical at every stage.

**Speed.** The Examples corpus (424 roots, with every dependency re-read and
re-analysed for each root) takes 4.5 s in a release build on an Apple M3, on a heavily
loaded machine.

## In the harness

The conformance harness now owns the corpus part of this check, for tlaplus/Examples,
the tlaplus `test/**` and `test-model/**` trees (corpus `tlaplus-tests`) and
CommunityModules:

```sh
cargo build --release -p dive-cli
dive-conformance run --impl dive --tier all \
  --corpus examples --corpus tlaplus-tests --corpus community-modules
```

- `dive-conformance`'s `dive` implementation answers each module from
  `dive analyze --output json`, with the golden oracle's classpath given as
  `--module-path`, in the order described above. See "`dive`" in
  `crates/dive-conformance/README.md`.
- The SANY goldens are the harness's own `-error-codes` runs (`kind: sany-module`), one
  per module, in the module's directory.
- The nightly `conformance` workflow runs this comparison.

The minimal cases (`tests/sany-cases/`) and the mutation campaign stay here. Mutants are
a natural deep-tier job for the harness.
