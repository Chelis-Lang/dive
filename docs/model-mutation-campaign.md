# Model-mutation campaigns

`scripts/model-mutation-campaign.py` compares `dive check` with TLC on seeded
behavioural mutants of pinned Examples models. The
[frontend campaign](frontend-campaign.md) mutates syntax and semantics and
compares parsing; this one keeps every mutant well formed and compares model
checking, so it reaches the failing and erroring paths that the committed
corpus, which mostly passes, rarely does. TLC is the oracle: no mutant needs a
hand-written expectation, and an equivalent mutant is still a valid
comparison.

```sh
cargo build --release --locked -p dive-cli -p dive-conformance
target/release/dive-conformance oracle fetch
target/release/dive-conformance corpus fetch --corpus examples
python3 -I scripts/model-mutation-campaign.py \
  --source-root "$DIVE_CACHE_DIR/git/examples/6a7b26a31c1a6aeec789b455e9bbfa3b3ad2728f" \
  --out "$TMPDIR/model-mutation" --seed 20261027 --models 64 --per-model 8 --jobs 3 --timeout 60
```

The output must not exist and must be outside the checkout and source cache.
`--prepare-only` writes the mutants without running either checker.
`python3 -I scripts/test-model-mutation-campaign.py` checks the edits.

## Selection and mutants

Models are Examples models in exhaustive mode whose manifest runtime is at
most five seconds, ranked by a hash of the seed and the model path. Mutation
sites come from the model's module and the same-directory modules it extends
or instantiates, since an Examples model is often a thin `MC` wrapper. Eight
classes are chosen round-robin in a seeded order:

| Class | Edit |
|---|---|
| boundary | `<` and `<=`, `>` and `>=` swapped: wrong only at the edge of a range |
| equality | `=` and `#` swapped (never an `EXCEPT` path's `=`) |
| constant | an integer literal plus or minus one |
| arithmetic | infix `+` and `-` swapped |
| logic | an infix `/\` and `\/` swapped |
| boolean | `TRUE` and `FALSE` swapped |
| drop-conjunct | one bulleted conjunct of a list replaced by `TRUE` |
| drop-disjunct | one bulleted disjunct of a list replaced by `FALSE` |

Comments and strings are never edited. Operator edits keep every bullet's
column, because junction lists are aligned by column; drops replace a whole
item. Each mutant is a byte copy of its specification directory with one
module edited and its own manifest. `selection.json` records every edit, and
preparing with the same inputs and seed is byte-identical.

The harness then generates TLC goldens for the derived corpus and runs
`dive check --compat=tlc` at one worker against them. `campaign.json` tallies
outcomes per class and lists each non-agreement with its edit.

## Results at seed 20261027

503 mutants of 64 models: 388 agree, 20 agree pending trace validation, 17
disagree, 25 unsupported, 1 timeout and 52 oracle errors.

- Five mutants make an action used under `WF_` fail with an evaluation error.
  TLC exits 255 (`GENERAL`); dive exits 75 (`TLC_NESTED_EXPRESSION`). The
  smallest reproduction is a `CHOOSE` with no satisfying element in an action
  named by `WF_vars`; the same error outside fairness exits 75 in both. (#502)
- Eleven mutants fail an `ASSUME`. dive matches the verdict, exit code and
  message, but its JSON `property` is empty while TLC's golden names the
  assumption's location, so the harness reports `violated_property`. No
  committed single-worker golden has an assumption failure.
- One liveness violation names fewer properties than TLC (#489).
- Unsupported results are named refusals: graph capture with `VIEW` (14),
  liveness planning (8), TLC error classes not yet reproduced (2) and a
  lazy function applied outside its domain (1).
- The timeout is a constant raised from 3 to 4: TLC took 56 seconds of the
  60-second limit.
- Two oracle errors are TLC timeouts. The other 50 are TLC output the
  classifier leaves unclassified on purpose: an exit code explained only by
  the context message 2103 after a `GENERAL` 1000
  (`crates/dive-conformance/src/tlc/result.rs`, rule 4). That is TLC's usual
  shape for an evaluation error while computing successors, so those error
  paths get no golden and are not compared (#436 proposes classifying one
  such family).
