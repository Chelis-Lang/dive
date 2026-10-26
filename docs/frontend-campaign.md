# Reproducible frontend campaigns

The deep conformance dispatch runs a separate frontend job. It combines both
vendored SANY-case directories with syntax, layout and semantic mutations of
the pinned Examples source. This supplements the committed corpus goldens;
it does not reuse the old scripts' verdict classifiers.

Dispatch with `tier=deep, scope=all` for the entire deep tier, or
`scope=frontend` to run this campaign alone. Nightly runs keep the standard
model/module checks and do not run mutations.

```sh
cargo build --release --locked -p dive-cli -p dive-conformance
target/release/dive-conformance oracle fetch
target/release/dive-conformance corpus fetch --corpus examples
python3 -I scripts/frontend-campaign.py \
  --source-root "$DIVE_CACHE_DIR/git/examples/6a7b26a31c1a6aeec789b455e9bbfa3b3ad2728f" \
  --out "$TMPDIR/frontend-campaign" \
  --seed 20261009 --sources 64 --per-file 4 --jobs 2 --timeout 15
```

Set `DIVE_CACHE_DIR` first. The output must not exist and must be outside the
checkout and source cache. `--harness` and `--dive-bin` select other release
builds. Python 3.9 or newer and its standard library suffice. `--prepare-only`
produces the selected inputs without running either implementation.

The source must match the actual Examples pin in `corpora.toml` and have a
clean git status. A seed ranks eligible roots and drives the existing mutator
functions. Modules over 30,000 bytes, invalid UTF-8, mismatched headers and
roots outside the source budget remain in the selection census with reasons.
Every selected source receives a bounded edit search for each mutation kind;
the requested and generated counts, including exhausted searches, are retained.

Each mutant keeps the original root filename in its own directory. Sibling
modules are byte copies, and only that root copy is overwritten. There are no
links, shared mutant directories, edits in verified caches or renamed roots
whose self-references accidentally resolve to another module. Both SANY-case
directories are included in full, with their helper modules. The derived local
corpus includes exactly those selected roots; its tree digest also covers the
copied dependencies.

Two distinct measurements are retained:

- **Full frontend:** the normal harness generates SANY goldens with a recorded
  reason, then compares `dive analyze` against them. Parsing, resolution,
  semantic/level checking and rejecting PlusCal-translation checks all count.
- **Root grammar:** a small reviewed Java driver calls the pinned
  `TLAplusParser.parse()` directly and compares with `dive parse`. It measures
  grammar acceptance and filename mismatch only. It never claims dependency,
  semantic or PlusCal acceptance.

This distinction matters for the malformed PlusCal comment cases
`PcalUnterminatedString` and `PcalEndWithoutBegin`. Their TLA+ grammar parses,
but SANY's translation validator rejects the first with INTERNAL_ERROR and
crashes on the second with an index of -1. A generic exception cannot establish
a grammar fact. The campaign records the direct parser result instead of
guessing from that exception. The full harness recognizes the specific
reported `SpecObj.validateParseUnit` validation error as a loading-phase
rejection; arbitrary exceptions stay unclassified.

The driver source and Java-17 bytecode are pinned by SHA-256. It runs with the
verified JRE and classpath already recorded by the SANY harness. Its build
recipe and scope are in `scripts/java/README.md`. A timeout, crash, malformed
process report, contradictory acceptance/exit or unknown result is a finding.
Every selected root must appear exactly once in each report, and every outcome
must be agreement for the campaign to pass. Nothing is excluded merely because
it fails the oracle.

Keep the entire output as evidence: selection census and source/mutator hashes,
derived sources and config, generated goldens, process logs, direct-grammar
rows and the aggregate `campaign.json`. Implementation and harness binary
hashes are recorded. Replaying preparation with the same inputs and seed must
produce byte-identical sources and selection metadata. Oracle timestamps and
process timing remain measurement metadata.

The first run at seed 20261009 selected 64 source roots and generated 768
mutants (256 of each kind), alongside all 340 vendored SANY-case modules.
Both measurements agreed on all 1,108 inputs, with no exhausted edit searches,
oracle errors, timeouts or disagreements. Preparation at the same seed was
byte-identical. This repeats a bounded campaign, not the original entire
12,000-plus-mutant survey. Future seeds remain new measurements.

The [model-mutation campaign](model-mutation-campaign.md) applies the same
seeded approach to model checking: well-formed behavioural mutants of Examples
models, compared with TLC.
