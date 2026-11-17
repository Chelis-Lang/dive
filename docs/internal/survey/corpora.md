# Corpora

Data gathered 2026-10-08 from shallow clones and `gh api`. Counts come from a
local walk of each clone at the commit given.

## Findings that affect the charter and ADRs

> **1. ADR 0005's description of Examples CI is slightly off.** It says the CI
> "classifies results by exit code (0, or 10–14)". `.github/scripts/tla_utils.py:265-273`
> maps only `0 success`, `10 assumption failure`, `11 deadlock failure`,
> `12 safety failure` and `13 liveness failure`. Any other code, including 14
> (`Assert`), becomes its number as a string and therefore fails to match any
> manifest `result`. The manifest schema has no "assertion failure" result.
> ADR 0005's `result` → exit-code mapping should list only 0 and 10–13.
>
> **2. Examples CI validates against the nightly TLC, not v1.7.4.** It downloads
> `http://nightly.tlapl.us/dist/tla2tools.jar` (`.github/scripts/linux-setup.sh`)
> and the *latest* CommunityModules, Apalache and tlapm. On 2026-10-08 the
> nightly jar is byte-identical to the rolling `v1.8.0` asset (`oracles.md`). The
> manifests' recorded numbers are therefore 1.8.0-era facts. ADR 0005's
> "oracle-drift" cross-check will fire heavily if goldens come from 1.7.4.

## tlaplus/Examples

| | |
|---|---|
| URL | https://github.com/tlaplus/Examples |
| HEAD | `6a7b26a31c1a6aeec789b455e9bbfa3b3ad2728f` (2026-10-03, "Bump CCF submodule to 8575ce8fc") |
| Licence | MIT (`LICENSE.md`: "All these TLA+ examples are licensed under the MIT License"). GitHub reports `NOASSERTION` because of the file format. Individual specs may carry their own headers. |
| Activity | pushed 2026-10-03, 1586 stars |

### Manifest schema (`manifest-schema.json`, verbatim)

```json
{
  "type": "object",
  "required": ["sources", "authors", "tags", "modules"],
  "additionalProperties": false,
  "properties": {
    "sources": {"type": "array", "items": {"type": "string"}},
    "authors": {"type": "array", "items": {"type": "string"}},
    "tags": {"type": "array", "items": {"enum": ["beginner", "intermediate", "ewd"]}},
    "modules": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["path", "features", "models"],
        "additionalProperties": false,
        "properties": {
          "path": {"type": "string"},
          "features": {"type": "array", "items": {"enum": ["pluscal", "action composition"]}},
          "proof": {
            "type": "object",
            "required": ["maxRuntimeMinutes"],
            "additionalProperties": false,
            "properties": {"maxRuntimeMinutes": {"type": "integer"}}
          },
          "models": {
            "type": "array",
            "items": {
              "type": "object",
              "required": ["path", "runtime", "mode", "result"],
              "additionalProperties": false,
              "properties": {
                "path": {"type": "string"},
                "runtime": {"type": "string", "pattern": "^(([0-9][0-9]:[0-9][0-9]:[0-9][0-9])|unknown)$"},
                "distinctStates": {"type": "integer"},
                "totalStates": {"type": "integer"},
                "stateDepth": {"type": "integer"},
                "mode": {
                  "oneOf": [
                    {"enum": ["exhaustive search", "generate", "symbolic"]},
                    {
                      "type": "object",
                      "required": ["simulate"],
                      "additionalProperties": false,
                      "properties": {
                        "simulate": {
                          "type": "object",
                          "required": ["traceCount"],
                          "additionalProperties": false,
                          "properties": {"traceCount": {"type": "number"}}
                        }
                      }
                    }
                  ]
                },
                "result": {"enum": ["success", "assumption failure", "deadlock failure", "safety failure", "liveness failure", "unknown"]},
                "workers": {"oneOf": [{"type": "integer", "minimum": 1}, {"enum": ["auto"]}]}
              }
            }
          }
        }
      }
    }
  }
}
```

(Whitespace compacted. The content is unchanged.)

### Counts at `6a7b26a`

81 `specifications/*/manifest.json` files, 424 modules, 233 models. 73 modules
have a `proof` entry. Features: `pluscal` 35, `action composition` 2. Tags:
beginner 18, ewd 5, intermediate 2.

| Mode \ result | success | safety failure | liveness failure | unknown | total |
|---|---|---|---|---|---|
| exhaustive search | 158 | 14 | 2 | 8 | **182** |
| symbolic (Apalache) | 45 | 1 | 0 | 0 | 46 |
| simulate | 1 | 2 | 0 | 0 | 3 |
| generate | 0 | 0 | 0 | 2 | 2 |
| total | 204 | 17 | 2 | 10 | 233 |

- No model records `assumption failure` or `deadlock failure`. The corpus does
  not exercise those verdicts, or `Assert` failures. `dive` needs its own
  minimal specs for them.
- Exhaustive with a recorded runtime of 30 s or less (ADR 0005 "small"): **140**
  (124 success, 14 safety, 2 liveness). Exhaustive with `runtime: unknown`: 10.
- 122 exhaustive models record `distinctStates`, `totalStates` and `stateDepth`.
- `workers` is set on 2 models (`1` and `4`). Everything else runs with
  `-workers auto`.
- Module dependencies (rough scan of `EXTENDS`/`INSTANCE` in manifest-listed
  modules): about 71 modules use TLAPS library modules (`TLAPS` 67,
  `FiniteSetTheorems`, `NaturalsInduction`, `SequenceTheorems`, …), about 44 use
  CommunityModules (`IOUtils` 11, `Functions` 9, `SequencesExt` 9, `CSV` 7,
  `FiniteSetsExt` 6, `SVG` 5, `Json` 3, …), 4 use `TLCExt`, and 2 use
  `Apalache`. **`dive parse` must resolve the TLAPS library, CommunityModules and
  `Apalache.tla` modules to accept the whole corpus.**

### How Examples CI invokes the tools (`.github/workflows/CI.yml`, `.github/scripts/`)

- **Dependencies** (`linux-setup.sh`): the TLC nightly jar from `nightly.tlapl.us`;
  Apalache `releases/latest` (fallback v0.52.2); `CommunityModules-deps.jar` from
  `releases/latest`; tlapm `main` tarball (library) and `1.6.0-pre` binaries;
  `tlauc` latest. **Nothing is pinned.** JDK 17 is used for TLC and JDK 21 for
  Apalache (`CI.yml:76-89`).
- **Matrix**: Windows, Linux and macOS × `unicode: true|false`, excluding
  macOS+Unicode (`CI.yml:53-61`). The Unicode legs convert every spec with
  `tlauc` (`unicode_conversion.py`), then check one "representative" model per
  (features, proof, mode) class (`check_small_models.py:99-122`). Apalache is
  skipped on those legs.
- **SANY**: `java -cp tla2tools.jar:apalache.jar:<spec dir>:<community>:<tlapm/library> tla2sany.SANY -error-codes <module>`.
  Pass means exit 0 (`parse_modules.py:42-64`). Every manifest module is parsed.
- **PlusCal**: `translate_pluscal.py` runs the translator on `pluscal` modules
  (skipping 3 byzpaxos files, tlaplus#906), then `git reset --hard`
  (`CI.yml:105-126`).
- **TLC** (`tla_utils.py:179-256`): `java [-enableassertions] -Dtlc2.TLC.ide=Github -Dutil.ExecutionStatisticsCollector.id=… -XX:+UseParallelGC -cp tla2tools:apalache:community:tlapm-lib [-Dtlc2.tool.impl.Tool.cdot=true] tlc2.TLC <module> -config <cfg> -workers <manifest workers or auto> -lncheck final -cleanup [-simulate num=N | -generate]`.
  It does **not** use `-tool`. Symbolic models run `apalache-mc check --length=5 --config=<cfg>`.
- **Result check** (`check_small_models.py:64-86`): the exit code is mapped as in
  finding 1 and compared with `result`. For exhaustive `success` models only, the
  counts are parsed from stdout with:
  ```python
  state_count_regex = re.compile(r'(?P<total_states>[\d,]+) states generated, (?P<distinct_states>[\d,]+) distinct states found, 0 states left on queue.')
  state_depth_regex = re.compile(r'The depth of the complete state graph search is (?P<state_depth>[\d,]+).')
  ```
  (`tla_utils.py:303-304`). Distinct and total are compared. **Depth is not
  compared** (commented out, citing tlaplus#883, `tla_utils.py:300-301`).
- **Timeouts**: hard limit of 60 s for TLC and 120 s for Apalache on "small"
  models (`check_small_models.py:48`). Models over 30 s are smoke-tested for 5 s
  and must still be running (`smoke_test_large_models.py:2-3,42`).
- **Selection**: "small" means `runtime <= 00:00:30` in **any** mode, symbolic
  included (`check_small_models.py:125-134`). ADR 0005 narrows this to
  exhaustive, which is our choice and not upstream's.
- **Skips**: `specifications/ewd840/EWD840.cfg` ("strange issue with parsing TLC
  output", `CI.yml:140`); `specifications/KnuthYao/SimKnuthYao.cfg` from smoke
  tests; `specifications/ewd426/SimTokenRing.cfg` on Windows (`CI.yml:169-173`).
  Several proofs are excluded from `tlapm --strict` (`CI.yml:191-205`).
- **`.ciignore`** lists the 4 submodule directories and
  `specifications/tcp/IndInv_apa{,_init}.cfg`. It is read by
  `check_manifest_files.py` and `generate_manifest.py`, but **not** by
  `check_small_models.py` or `parse_modules.py`, which are driven by the
  manifests alone.

### Submodules (`.gitmodules`; SHAs from `gh api …/contents/<path>`)

| Path | Upstream | Pinned SHA |
|---|---|---|
| `specifications/CCF` | microsoft/CCF | `8575ce8fcea5919b6c74b7a282b8ca641cf37aab` |
| `specifications/azure-cosmos-tla` | tlaplus/azure-cosmos-tla | `4b8785a36ed16faa2cd1bd78b89e224d71711ee3` |
| `specifications/BlockingQueue` | lemmy/BlockingQueue | `68715979666344cbcc66fa621b27039f281bdd5c` |
| `specifications/microwave` | lucformalmethodscourse/microwave-tla | `5ebaed2cc7d69fef578e950ae142c60cba958f87` |

None of the four trees contains a `manifest.json` at the pinned SHA (recursive
tree listing via `gh api`). All are in `.ciignore`. **They are not part of the
manifest corpus.** A non-recursive clone loses nothing for conformance
purposes.

## tlaplus/CommunityModules

| | |
|---|---|
| URL | https://github.com/tlaplus/CommunityModules |
| HEAD | `78b98ae9939d2fe7a6615e9a4b21dd6c23ef8ff3` (2026-10-04, "Make the AntiFunction override total on non-injective functions (#135)") |
| Licence | MIT (`LICENSE`, "Copyright (c) 2019 TLA+"). Bundled third-party jars in `lib/` (gson, jgrapht, jungrapht, slf4j, commons-lang3, commons-math3) have their own licences. |
| Releases | One immutable release per master build, tagged by timestamp. Latest `202610040242` → `78b98ae`, published 2026-10-04, with GitHub digests: `CommunityModules-deps.jar` `sha256:dddb19c3d7596913d92c43073b60d1408ac8f6d86716af4d19dd6861a95bfb7d`, `CommunityModules.jar` `sha256:b7fcd727fc5135a628e56d5015651c74ba70681bbaf88df1cd59e9afa64215ad`. |
| Content | 39 `.tla` in `modules/` (BagsExt, Bitwise, Combinatorics, CSV, DifferentialEquations, DyadicRationals, FiniteSetsExt, Folds, Functions, Graphs, GraphViz, HTML, IOUtils, Json, Quorum, Relation, SequencesExt, ShiViz, Statistics, SVG, UndirectedGraphs, VectorClocks, plus `*Theorems`/`*_proofs`) |
| Java overrides | `modules/tlc2/overrides/`: 19 `.java`, **66 `@TLAPlusOperator` methods**. The heaviest are SequencesExt 18, IOUtils 8, Json 7, Bitwise 5, Functions 4, CSV 4 and SVG 4. They are registered by `TLCOverrides.get()` (`TLCOverrides.java`), which TLC discovers as the first `tlc2.overrides.TLCOverrides` on the classpath (`tlc2/tool/impl/SpecProcessor.java:2188-2195` in tlaplus). |
| Tests | `tests/`: 30 files, including `AllTests.tla`/`.cfg` with **791 `ASSUME`s** across the `*Tests.tla` modules. TLC evaluates these, and a false one yields exit 10. |
| Build | `build.xml:27` compiles against the `v1.8.0` release jar with `source/target 1.8` (`:31-33`). |

**How a spec uses it**: put the jar on TLC's classpath
(`java -cp tla2tools.jar:CommunityModules-deps.jar tlc2.TLC …`, README line 57),
or place it next to `tla2tools.jar`, where the jar manifest's `Class-Path`
picks it up (`oracles.md`). The `.tla` files are loaded from the jar's root by
SANY's resolver. `IOUtils` exposes `IOExec`, `IOEnvExec` and `IOExecTemplate`
(`IOUtils.java:250-317`), which run arbitrary processes. The `-deps` variant
bundles gson and friends. Without it, `Json` overrides are disabled with a
console message (`TLCOverrides.java`).

## tlaplus/ValidationTestSuite

| | |
|---|---|
| URL | https://github.com/tlaplus/ValidationTestSuite |
| HEAD | `fe99cff1ca48535bdbfc99fd95bd2706f056c12f` (2026-09-10) |
| Licence | **Apache-2.0** (`LICENSE.md`; SPDX headers "Copyright (c) 2022-2026 NVIDIA CORPORATION & AFFILIATES"). This is not MIT, so attribution and NOTICE rules apply if we vendor generated specs. |
| What | NVIDIA's ISO 26262 tool-qualification generator for TLC (README). Python (`testgen/`) builds specs from **pairwise combinations of 100+ TLA+ features** (`testgen/*.json` operator tables, `testgen/feature.py`). It runs TLC (workers 1, 2, auto) and Apalache, and cross-checks them. A separate generator covers `SYMMETRY`×config interactions (`symmetry.md`, `testgen/SymmetrySpec.tla`). Anomalous conditions are tested in `testgen/AnomalousSpec.tla`. |
| Format | The repository holds the generator, not the specs. Specs and results appear in release artifacts: `reports.zip` of about 370 MB per release, for example `sha-fe99cff1…` (2026-09-10) and `sha-19944506…` (2026-05-21). CI regenerates every 14 days against the nightly jar (`.github/workflows/main.yml:6-7,51-65`). The TLC jars are stored in Git LFS (`tools/1.7.2/`, `tools/2026.04.27.203507/`, `.gitattributes`). |
| TLC invocation | `java -XX:+UseParallelGC -cp <jar> tlc2.TLC -fp <random 0..130> -lncheck final -messagesAsErrors <codes> -metadir <tmp> … -config <cfg> <spec>` (`testgen/runner.py:185-202`). The full exit-code table is at `runner.py:25-50`. |
| Deviation log | `explanation.yaml`: 84 explained TLC-vs-Apalache divergences, for example CHOOSE with no witness (TLC errors, Apalache returns an arbitrary value) and tuple patterns in CHOOSE. |
| Usability | High value as a **generated feature-pair corpus** of small specs with known TLC outcomes, and as a ready-made list of semantic corner cases. It needs Python 3.9, JDK 17 and, on Linux, bubblewrap to regenerate. Consuming a release `reports.zip` avoids running their code. |

## Other corpora worth pinning

| Corpus | Pin (2026-10-08) | Licence | Why |
|---|---|---|---|
| tlaplus/tlaplus `test-model/`, `test/tla2sany/**` | `94d0c5070852b37b801bb8998c436383ab7ee37b` | MIT | TLC regression specs and SANY syntax/semantic/error corpora (`oracles.md`) |
| tlaplus/tlapm `library/` | `d8b9747a1dce9b4816bcc1ff130ccf7a3af2ce37` (2026-10-06) | BSD-2-Clause | `TLAPS.tla`, `*Theorems.tla`: needed to parse about 71 Examples modules |
| apalache-mc/apalache `src/tla/` and `test/tla/` | `8677897cd394459a76311759a1d1442a2f50089d` (2026-10-03); release v0.62.3 | Apache-2.0 | `Apalache.tla`, `Variants.tla`, `Option.tla` (2 Examples modules); `test/tla/` has 292 entries of typed corner-case specs |
| tlaplus-community/tree-sitter-tlaplus `test/corpus/` | `add40814fda369f6efd989977b2c498aaddde984` | MIT | 376 syntax tests in 60 files, including `unicode/`, `pluscal/`, `nonstandard/` (`adjacent.md`) |

## Recommended pins

| Corpus | Commit | Notes |
|---|---|---|
| tlaplus/Examples | `6a7b26a31c1a6aeec789b455e9bbfa3b3ad2728f` | Non-recursive clone; submodules are not in the manifest corpus |
| tlaplus/CommunityModules | `78b98ae9939d2fe7a6615e9a4b21dd6c23ef8ff3` (release `202610040242`; deps jar `dddb19c3…`) | Pin the jar by SHA-256 as well as the source |
| tlaplus/ValidationTestSuite | `fe99cff1ca48535bdbfc99fd95bd2706f056c12f` (release `sha-fe99cff1…`) | Consume `reports.zip`; re-host it, since it is about 370 MB |
| tlaplus/tlaplus (tests) | `94d0c5070852b37b801bb8998c436383ab7ee37b` | Same commit as the 1.8.0 oracle snapshot |
| tlaplus/tlapm | `d8b9747a1dce9b4816bcc1ff130ccf7a3af2ce37` | `library/` only |
| apalache-mc/apalache | `8677897cd394459a76311759a1d1442a2f50089d` | `src/tla/` modules; `test/tla/` optional |
| tree-sitter-tlaplus | `add40814fda369f6efd989977b2c498aaddde984` | `test/corpus/` |

## Implications for dive

- The Examples manifests give 140 small exhaustive models and 182 exhaustive
  models overall. Neither set contains a deadlock, assumption or `Assert`
  verdict. `dive` needs a hand-written supplement covering those verdicts, plus
  evaluation errors and overflow, with goldens generated by the harness.
- Fix ADR 0005's exit-code wording (0 and 10–13), and record that upstream's
  manifest numbers come from the nightly (1.8.0) build, so 1.7.4 drift is expected.
- The harness must supply the module search path that Examples CI uses: the spec
  directory, CommunityModules, the tlapm `library/` and Apalache's `src/tla`
  modules. A `dive parse` without them will spuriously reject about 115 modules.
- Model Unicode as a first-class input. Examples CI converts the whole corpus
  with `tlauc` and re-checks it, and `dive` should run the same metamorphic leg.
- Mirror upstream's skips (`EWD840.cfg`, `SimKnuthYao.cfg`, `SimTokenRing.cfg`
  on Windows) as `out-of-scope` with a reason, never as a silent drop.
- `IOUtils!IOExec*` executes host processes. `dive` should refuse these by
  default (`unsupported` or an explicit opt-in), which keeps runs hermetic.
- The 791 `ASSUME`s in CommunityModules' tests are a cheap operator-level oracle
  for `dive-stdlib`. Run them through the harness, which treats a false
  assumption as exit 10 with message 2104.
- ValidationTestSuite is Apache-2.0. Before vendoring any generated spec into
  `conformance/`, record its provenance and keep it under a directory with its
  own licence notice.
