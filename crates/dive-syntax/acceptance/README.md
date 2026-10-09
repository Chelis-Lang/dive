# Parse acceptance against SANY

These scripts measure whether `dive-syntax` accepts exactly the modules that the pinned
SANY **parser** accepts. They produce a confusion matrix (both accept, both reject,
dive-only accept, SANY-only accept) per corpus, and they regenerate the verdict files
that the tests check against:
- `tests/corpus/sany-verdicts.txt`
- `tests/sany-cases/verdicts.txt`

Once the conformance harness (`dive-conformance`) produces SANY goldens, it should own
this check. See "Porting to the harness" below.

The scripts use only the Python 3 standard library. Run them with `python3 -I`, from a
directory that holds no downloaded files, and pass downloaded paths as arguments.

## Inputs

**Oracle.** Use the jar and JRE pinned in `conformance/oracles.toml`, and check both
sha256s before use:
- `tla2tools.jar`, sha256 `7beec0f0…ea78d`. It is a private release asset:
  `gh release download oracle-tlc-2026.10.06.014338 -R Chelis-Lang/dive -p tla2tools.jar`.
- Temurin JRE 17.0.20.1+1.

**Corpora.** Make shallow clones at the pins in `conformance/corpora.toml`, one empty
directory per clone. For example:

```sh
git init -q examples
git -C examples fetch -q --depth 1 https://github.com/tlaplus/Examples 6a7b26a31c1a6aeec789b455e9bbfa3b3ad2728f
git -C examples checkout -q FETCH_HEAD
```

Clone these the same way:
- `tlaplus/CommunityModules` at `78b98ae9…`;
- `tlaplus/tlaplus` at `94d0c507…`, which provides `test/**`, `test-model/**` and the
  syntax corpus;
- optionally `tlaplus/tlapm` at `d8b9747a…`, for `library/` on SANY's path.

## Steps

1. **List files.** Put one path per line in a `NAME.list` file, for example
   `find examples -name '*.tla' | sort > examples.list`.
2. **SANY verdicts.**

   ```sh
   python3 -I run_sany.py "$JAVA" tla2tools.jar sany-NAME.jsonl "$LIB" $(cat NAME.list)
   ```

   - `$LIB` is a `:`-separated `TLA-Library` path, for example
     `community/modules:tlapm/library`.
   - `run_sany.py` runs `tla2sany.SANY -s` (no semantic analysis).
   - SANY stops at the first fatal error, **including a missing dependency**. The
     script therefore restarts the batch after the failing file. It classifies each
     file only from its own output segment:
     - `reject`: SANY printed `Could not parse module M from file F` for *this* file.
       This covers parse errors and lexical errors.
     - `name_mismatch`: the module parsed, but the file name differs from the module
       name. SANY reports this in its parse phase; dive reports it as `DIVE-S0006` from
       `Parse::check_file_name`. It counts as *accept* for parser acceptance.
     - `accept`: anything else. Dependency failures happen after the root parse
       succeeded.
     - `crash`: no verdict; the file is excluded and reported.
3. **dive verdicts.**

   ```sh
   cargo run --release -p dive-syntax --example acceptance -- --list NAME.list > dive-NAME.jsonl
   ```

   This also prints total bytes, time and throughput on stderr.
4. **Compare.**

   ```sh
   python3 -I compare.py sany-NAME.jsonl dive-NAME.jsonl
   ```

   It prints the matrix and lists each disagreement with both tools' messages.

**Syntax corpus.**
- `python3 -I extract_corpus.py <tlaplus>/tlatools/org.lamport.tlatools/test/tla2sany/corpus OUT`
  writes one `OUT/NNNN/Test.tla` per test case, plus `OUT/index.json`.
- Run steps 1 to 4 on those files.
- `python3 -I corpus_verdicts.py OUT/index.json sany-corpus.jsonl ../tests/corpus/sany-verdicts.txt`
  regenerates the vendored verdicts.

**Minimal cases.**
- `python3 -I make_cases.py OUT` writes the specs in `tests/sany-cases/` (copy the
  `.tla` files across).
- After step 2, `python3 -I verdicts.py sany-cases.jsonl OUT ../tests/sany-cases/verdicts.txt`
  regenerates the verdicts.

**Mutation campaign.**
- `python3 -I mutate.py OUT SEED N [--layout] FILE...` writes `N` mutants per file:
  - token deletion, duplication and swap;
  - operator replacement and insertion;
  - re-indentation and line joining.

  The module header is never mutated. `--layout` favours tab substitution and CRLF/CR
  line endings, which change JavaCC's column counting, and stacks up to three edits.
- Run steps 1 to 4 on the mutants.

## Results (2026-10-08, pinned oracle)

| Corpus | Files | Both accept | Both reject | dive-only | SANY-only |
|---|---|---|---|---|---|
| tlaplus/Examples `6a7b26a` | 424 | 424 | 0 | 0 | 0 |
| CommunityModules `78b98ae` | 78 | 78 | 0 | 0 | 0 |
| tlaplus `test/**` + `test-model/**` `94d0c50` | 808 | 804 | 4 | 0 | 0 |
| tlaplus syntax corpus `94d0c50` | 355 | 326 | 29 | 0 | 0 |
| tree-sitter-tlaplus `test/corpus` `add40814` (includes PlusCal and nonstandard cases) | 389 | 362 | 27 | 0 | 0 |
| `tests/sany-cases` | 176 | 111 | 65 | 0 | 0 |
| Mutants, wave 1 (corpus + small test modules; seeds 11, 22, 33; 1 SANY crash excluded, see below) | 3,482 | 1,289 | 2,193 | 0 | 0 |
| Mutants, wave 2 (corpus + Examples/CommunityModules under 30 KB; seeds 44, 55, 66) | 4,758 | 2,422 | 2,336 | 0 | 0 |
| Mutants, layout wave (`--layout`: tabs, CRLF/CR, indentation, up to three edits; seeds 77, 88) | 3,176 | 2,095 | 1,081 | 0 | 0 |

One SANY run crashed (`java.lang.ArrayIndexOutOfBoundsException`) and is excluded: a
wave 1 mutant of Examples `LearnProofs/AddTwo.tla` whose only edits are inside comments.
One of them corrupts the `\* BEGIN TRANSLATION` marker. The crash is in the PlusCal
translation validation that SANY runs during its parse phase (it printed "Labels added."
first), not in the TLA+ parser. dive accepts the file, as SANY's parser does. Validating
PlusCal translations is `dive-pluscal`'s job.

**Performance.** These are `examples/acceptance.rs` parse times, release build, Apple M3,
single-threaded:
- Examples, 424 modules, 3.0 MB: 0.095–0.10 s over three runs, 28–30 MiB/s;
- tlaplus tests, 808 modules, 5.6 MB: 0.29 s.

The largest file, `test-model/Bug156/TE.tla` (2.9 MB of generated trace), takes about
0.19 s on its own.

## In the harness

The conformance harness now owns the corpus part of this check, for tlaplus/Examples,
the tlaplus `test/**` and `test-model/**` trees (corpus `tlaplus-tests`) and
CommunityModules:

```sh
cargo build --release -p dive-cli
dive-conformance run --impl dive --sany-phase parse --tier all \
  --corpus examples --corpus tlaplus-tests --corpus community-modules
```

- It needs no separate `-s` goldens. The parse-phase verdict of each file (`accept`,
  `name-mismatch`, `reject`) is derived from the full SANY goldens with the same rules
  as `run_sany.py`:
  - a failure counts only for the file whose `Could not parse module <M> from file`
    SANY printed;
  - `File name '<M>' does not match` is `name-mismatch`.

  This is sound because SANY parses the root file first and stops at the first
  parse-phase failure, and the golden keeps SANY's text from the first error marker
  on. A golden that does not determine the verdict is reported as `oracle-error`.
- It compares that verdict with `dive parse --output json`. `accepted`, with
  `DIVE-S0006` as the only error, is `name-mismatch`.
- The nightly `conformance` workflow runs this comparison.

The syntax corpus, the tree-sitter corpus, `tests/sany-cases/` and the mutation
campaign stay here. Mutants are a natural deep-tier job for the harness.

## Deep campaign runner

The deep conformance dispatch now runs both SANY-case directories and a
deterministic syntax/layout/semantic mutation subset through the pinned
harness. `scripts/frontend-campaign.py` preserves a complete selection census,
original module names and byte-copied dependencies in isolated directories.
Full frontend and direct root-grammar observations remain separate, including
PlusCal validation failures. See `docs/frontend-campaign.md`; these legacy
classifiers are not used by the new job.
