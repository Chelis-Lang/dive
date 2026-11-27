# Source-checkout tools

These drivers run from a dive source checkout and are not part of the
installed `dive` binary. They are research tools for symmetry evidence and
for compiling a model's successor function. The installed checker's options
are in the [user guide](../user/checking.md).

## Compare a declared carrier symmetry on the original files

The source-checkout driver takes an unmodified TLA+ module and configuration. Choose a constant bound to two, three or four distinct untyped model values that you expect to be interchangeable. This example uses the original TCommit specification and its three resource managers.

```sh
python3 scripts/perf/scale/checked_orbit.py TCommit.tla \
  --config TCommit.cfg --carrier RM --dive /path/to/dive --workers 8
```

By default the driver runs two exhaustive exact checks on private copies of the inputs. One uses a generated module extending the original with all pair transpositions of the carrier. The other checks the original files directly. A successful `verified-observables` result means the original reached a successful verdict and its distinct count, generated count and depth matched the expanded orbit totals and quotient depth. This comparison applies to the listed source closure, configuration and properties. Different labeled graphs can have the same counts. On failure, each reported counterexample is replayed against the original module and configuration.

Use `--assume-equivariance` for a single reduced check when the model author establishes that relabeling preserves the initial states, transitions, constraints, properties and transition multiplicities. All relevant expressions must remain defined independently of enumeration order. Dive checks that adjacent swaps preserve every configured constant. Its resolved-source observer audit checks for effects, opaque operators and `CHOOSE` expressions whose selected witness may distinguish two labels. For example, `CHOOSE w \in Writers : TRUE` refuses for a nontrivial writer symmetry. The preflight covers the lowered user-file definitions and can also refuse an unused definition. A successful reduced check in this mode reports `conditional-orbit`.

`--prepare-only` returns an unsigned input manifest with the module closure hashes, configuration digest, exact adapter source and configuration suffix. A separate client can verify the original bytes and submit the assembled request through its own checker. The receiver needs its own authority for any cached verdict. A source or configuration edit requires preparing the current inputs again. `dive analyze MODEL.tla --orbit-observers --output json` exposes the preflight's choice domains and refusal locations.

Exit 0 means the selected comparison or certificate succeeded. Exit 1 carries a replayed counterexample, exit 2 means refusal or disagreement, and exit 3 identifies a prepared request or a successful conditional result. `--seconds` limits each check, and `--max-memory` sets the checker's memory limit. [Flint and Lamport controls](../../conformance/reports/perf/2026-11-22-checked-orbits.json)

## Certify interchangeable model values

Use `--certify-equivariance graph` when you need evidence that every reachable labeled transition survives renaming. This source-checkout driver supports pure safety models with two to four distinct untyped model values in a literal configured carrier. The graph method handles ordinary compound values, translated process tables, duplicate transitions, and configured invariant and step obligations. It requires a complete original search within the supplied limits. Constraints, prior reductions and liveness yield a refusal.

Build the helper from the same checkout as the checker, then run it on the original module and CFG. For example, from the checkout root with the released transaction-commit files in `models/`:

```sh
cargo build --release --locked -p dive-cli --bin dive
cargo build --release --locked -p dive-api --example orbit_graph --example symbolic_input
python3 scripts/perf/scale/checked_orbit.py models/TCommit.tla \
    --config models/TCommit.cfg --carrier RM --certify-equivariance graph
```

The first proof runs both the original safety check and an exact capture of its reachable graph. For each adjacent carrier swap it verifies that every state maps into the graph, that the mapping is its own inverse, and that each initial state and each mapped successor occurs with the same multiplicity. Checking each outgoing multiset also covers swaps that fix the source. The fresh reduced search must agree with the original on verdict, expanded distinct and generated counts, and depth. A violation carries a word independently replayed against the original files. Stops, diagnostic disagreement and failed conjugacy yield exit 2. Successful certification returns exit 0 and `status: certified-orbit`.

For a model with a supported finite layout, select `--certify-equivariance finite --type-invariant TCTypeOK`. Install the optional [symbolic checker dependencies](../../scripts/symbolic/requirements.txt) in the Python environment that runs the driver. This method proves the finite initial occurrences, guarded updates with their multiplicities, invariant predicates, and supported boxed step predicates commute with the generators. It also checks the original directly on first use to preserve observable diagnostics. Ambiguous whole-function comparisons, unconstrained state coordinates, and unsupported value shapes or operators refuse by name. Use `--proof-memory-mib` to bound the helper address space and `--seconds` to bound each individual proof or check.

To reuse a proof in another local process, create a private 32-byte key once and retain it under your control. Python 3.11 or newer is required.

```sh
export DIVE_ORBIT_PROOF_KEY="$PWD/orbit.key"
python3 -c 'import os,sys; f=os.open(sys.argv[1],os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600); os.write(f,os.urandom(32)); os.close(f)' "$DIVE_ORBIT_PROOF_KEY"
python3 scripts/perf/scale/checked_orbit.py models/TCommit.tla \
    --config models/TCommit.cfg --carrier RM --certify-equivariance graph \
    --certificate-cache "$PWD/orbit-proofs"
```

Run the final command again to see `certificate.reuse: authenticated`. Every invocation rebinds the source and runs the reduced search. The HMAC covers the complete source and CFG identity, group, checker, proof implementation, worker count, and proof result. An edit requires a new proof. Undo can reuse the original. A damaged receipt or a different private key causes reproof. Possession of the key grants authority to sign a reusable proof. Give another checker the prepared input bytes and let it establish its own verdict and cache authority. [Complete edit cycle, memory and separate Sail measurements](../../conformance/reports/perf/2026-11-22-certified-orbits.json).

## Build a source-bound native successor worker

For repeated checks of a growing program, the Linux source checkout includes an experimental compiler for Core's successor control flow. It accepts an unmodified module and configuration. Each generated worker is an executable with the ordinary `dive check` interface. It handles guards, assignments, scopes and finite existential choices using the current Core evaluator. Unsupported generator nodes continue in the ordinary generator, including pending conjunctions. Exact and fingerprint storage retain their usual guarantees.

Preparation needs Python 3.11+, the project's Rust toolchain, Git, tar and strip. Give it a private build cache outside the checkout. The first preparation copies the committed Dive tree and builds a bootstrap checker and a model-specific executable for the current Linux platform. On the measured Flint run, preparation took 38.39 s and peaked at 1.51 GiB of charged process-family memory. The normal installed checker starts without this build.

```sh
python3 scripts/perf/scale/native-successors/prepare.py Model.tla \
  --config Model.cfg --cache "$HOME/.cache/dive/successor-builds" --jobs 4 > prepared.json
worker=$(python3 -c 'import json; print(json.load(open("prepared.json"))["binary"])')
"$worker" check Model.tla --config Model.cfg --workers 8 --seed 0 --fp 0 --output json
```

Rerun preparation after an edit. It resolves the current input, exports a fresh bound plan before exploration and checks whether the same generated instructions still apply. The receipt records the original module hashes, configuration hash, kernel identity, build time and executable checksum. Local builds and cache manifests are trusted. Colocated source modules and pure Core action plans are supported. Oversized, effectful or unavailable plans produce a named refusal from preparation.

For frequent full searches, add `--optimize throughput` to preparation. Install the matching LLVM tools once with `rustup component add llvm-tools-preview` in the source checkout. The default, `--optimize build-time`, favors a short first preparation. Throughput mode builds an instrumented worker, checks a private snapshot of the original TLA+ and CFG, then uses LLVM’s execution counts to build a worker for the standard Rust target with thin link-time optimization. The sample uses up to eight workers (limited by `--jobs`), 14 levels, 75,000 states, 15 seconds and 2 GiB. Only the subsequent invocation of `"$worker" check` answers the user’s check.

The JSON receipt separates the current `inputs` from `training.training_inputs`. After a compatible property or guard edit, the worker can retain the original training profile while checking the new program. The receipt reports `built: false` on reuse, the executable and profile hashes, the training outcome, and separate build and sampling times. The two build modes have distinct cached executables.

On eight physical cores, original Flint took 2.04–2.08 s per search with the quick build and 1.50–1.51 s with the trained worker. Full-state equality took 2.52 and 1.90 s respectively. An edit growing the graph to 7,387,957 states took 33.85 and 24.61 s. With the bootstrap already available, training and compilation took 74.42 s, versus about 14 s to compile the quick worker. At those measured search times, the extra preparation pays back after roughly 108 original searches or seven growth searches sharing one build. The growth edit reused the trained binary in 0.23 s. A separate process-family measurement put the trained search at 393 MiB and 24.80 s, compared with 1.40 GiB and 21.33 s for already compiled tla-mc. [Per-run timings, cold and edited peer comparisons, memory and correctness checks](../../conformance/reports/perf/2026-11-23-trained-source-workers.json).

On the released three-floor Elevator liveness model, one worker checked 50,653 states in 1.71 s with the quick build and 1.26–1.27 s when trained on Elevator. A worker trained on Flint also completed Elevator in 1.40–1.41 s using ordinary successor traversal. Its compiled Flint actions had a different shape and recorded zero calls. On this unrelated model, an earlier worker built with ordinary full LTO and no training also took 1.39–1.41 s. [Worker scaling and transfer measurements](../../conformance/reports/perf/2026-11-23-trained-source-workers.json).

At check time, the executable compares the entire bound generator structure with its compiled shape. A different shape, debugging, unbounded simulation or a non-exhaustive sink uses ordinary traversal. Current predicates and values are evaluated anew even when the control shape still matches. A fresh search reconstructs the graph, checks the configured obligations and retains their diagnostics. For a matched control set `DIVE_DISABLE_NATIVE_SUCCESSORS=1`. For a separate coverage run, `DIVE_NATIVE_SUCCESSOR_METER` can name a fresh file recording power-of-two counts of calls to compiled actions. A separate Sail installation accepts the underlying TLA+ inputs and maintains its own cache.

Flint's property edit completed in 3.30 s including preparation, against 23.10 s for compiled tla-mc. Unchanged warm search on the same labeled graph took 3.07–3.18 s against 1.49 s. [Original and edited measurements, fallback and trace controls](../../conformance/reports/perf/2026-11-22-source-native-successors.json).

Ordinary `dive check` recognizes repeated updates to distinct fields of the same finite actor record, such as `[w EXCEPT ![a].pc = next, ![a].seen = stamp]`. It preserves source order, nested paths and the old value bound to `@`. A missing key, a repeated field or a different runtime shape uses ordinary evaluation. The original Flint check took 2.94–3.02 s with this lowering, and the optional compiled worker took 2.62–2.63 s plus its binding check. Compiled tla-mc remained faster on unchanged warm checks. [Full equality, growing models and edit-cycle measurements](../../conformance/reports/perf/2026-11-22-actor-table-rows.json).

The prepared worker can read one actor label for several action alternatives, pass the checked position into an update, iterate small closed scalar domains, and update only the changed cells of a guarded table of sets. Each new process binds those operations to its current source and configuration. A missing field, different domain or unrecognized value operation uses the ordinary evaluator. The Flint guard edit took 36.25 s including preparation and search against 43.28 s for the peer's edited source, with both checking all 7,387,957 states. The already compiled peer completed the same check in 22.78 s. [Matching cold and warm runs and exact-store comparisons](../../conformance/reports/perf/2026-11-22-source-value-blocks.json).

To see which operations run, set `DIVE_NATIVE_BLOCK_METER` to a new writable file when invoking the prepared binary. It records a lower bound for each operation at powers of two. For a same-worker comparison, set `DIVE_DISABLE_NATIVE_BLOCKS=1` to use the previous generated continuations, or `DIVE_DISABLE_NATIVE_SUCCESSORS=1` to use the ordinary Core generator.
