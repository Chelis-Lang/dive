# Compatibility with TLC

Pass `--compat=tlc` to get TLC's behaviour for integer bounds, comparison and
exploration order, fingerprints, seed handling and run defaults. The CLI also
reads `DIVE_COMPAT=tlc`. Rust requests have compatibility fields, and
low-level checking uses `CheckOptions::tlc(fp_index, seed)`. Core is the only
engine, so `dive check Spec.tla --compat=tlc` runs on Core.

The profile uses the TLC-compatible switches dive already has. It does not
promise exact TLC fidelity: known gaps are listed with reproducers under
[compat gaps](../tlc-differences.md#compat-gaps), and an unsupported feature
refuses by name.

| Behavior | Dive default | With `--compat=tlc` |
| --- | --- | --- |
| Integers | Unbounded | 32-bit range with overflow errors |
| CHOOSE/value exploration | Canonical order | TLC's encounter-dependent ordering |
| Structural comparisons | TLA+ comparisons, including ones TLC refuses | TLC's comparison/error behavior |
| Safety workers | Automatic parallel width | One by default; an explicit width is reported |
| Fingerprints | Native combination | TLC polynomial scheme, selectable with `--fp` |
| Symmetry with temporal properties | Unreduced checking over the whole run ([symmetry with liveness](../tlc-differences.md#d-005-symmetry-with-liveness-uses-the-full-state-space)) | `--symmetry-liveness=tlc` asks for TLC's quotient semantics and refuses by name; `unreduced` is labelled `tlc+overrides` |
| Temporal checking | Deterministic checks at completed levels | One final check after exhaustion; TLC's wall-clock schedule is not reproduced |
| `--lncheck final --lasso short` | Refuses by name (exit 3) when the model has temporal properties | Same |
| Wall-clock budget (`--time-limit`) | None unless set; a set limit ends in `stopped`, exit 5 | None, as in TLC |
| Memory budget (`--max-memory`) | Half of physical memory, then `stopped`, exit 5 | Same ceiling; TLC itself dies with `SYSTEM_OUT_OF_MEMORY` |

A named opt-in on top of the TLC profile reports the profile as
`tlc+overrides`: for example `--checkpoint-counters retain`,
`--exit-codes native`, or an explicit time limit. Read the resolved profile and
options in the report instead of inferring compatibility from one flag.

`--compat=tlc` never grants host access. Every profile starts with no grants,
and `--allow-io=env,files,exec` grants categories explicitly. Core does not
implement external inputs: with the grant, a specification that reaches
`IOUtils!IOEnv` or `IOUtils!Deserialize` refuses by name with exit 3. See
[host inputs](modules.md#host-inputs-and-explicit-grants).

## Action composition

Action composition (`A \cdot B`) uses `--action-composition=tla` by default and
counts only composed successors. Core supports a pure enumerable first stage
that assigns every variable, followed by a deterministic tail with one
assignment `v' = e` per variable, where `e` is unprimed, optionally inside
conjunctions and `LET`. The tail reads the intermediate state, and the reported
edge connects the original state to the final state. Duplicate final candidates
still count.

These refuse by name: incomplete intermediate states, effects, nested or
recursive composition, unproved tails, and generation with pending conjuncts or
preassigned variables. Predicates on complete steps use the same composition
relation.

```sh
dive check crates/dive-api/tests/fixtures/core-composition/specs/Reduction/Reduction.tla \
  --config crates/dive-api/tests/fixtures/core-composition/specs/Reduction/Reduction.cfg \
  --workers 1 --seed 0 --fp 0
```

This succeeds with 3 distinct states, 7 generated states and depth 2. Pinned
TLC with its experimental composition switch counts 13 generated states,
including six intermediate states.

`--compat=tlc` turns composition off. Add `--action-composition=tla` to keep
composition with the profile's other switches. Core refuses
`--action-composition=tlc` and its `-Dtlc2.tool.impl.Tool.cdot=true` spelling,
which ask for TLC's intermediate accounting. An explicit composition option
takes precedence over the profile regardless of flag order, and
`options.action_composition` reports the resolved setting. See
[D-022](../tlc-differences.md#d-022-action-composition-works-by-default-with-its-tla-meaning).

## Seeds and replay

Record the reported seed and fingerprint settings to replay a run. The
independent validator refuses a random trace whose draw order it cannot align.
DFID in the TLC profile has extra seed rules, described in [DFID](dfid.md).

## Temporal checking in the TLC profile

An eligible `--compat=tlc` request checks temporal properties once after
exhaustion, with TLC's value semantics and a short, independently validated
lasso. The report records the final/short mode and keeps profile `tlc`. Dive
does not reproduce TLC's exact lasso choice, so there is no `--lasso tlc`
mode; asking for it is a usage error.

The default profile uses the same analyzer at deterministic completed-level
boundaries ([periodic liveness checking](../tlc-differences.md#d-014-periodic-liveness-checking-has-a-replayable-schedule)).
[Temporal checking](temporal.md#ordinary-checking-in-the-tlc-profile) gives the
eligibility rules, including fingerprint 0 and resident memory, and the
limits. Temporal routes outside them refuse by name. TLC's wall-clock periodic
trigger is not emulated.

## Where the details live

The [differences log](../tlc-differences.md) holds the minimal examples,
measurements, default choices and matching settings for every difference. Its
compat-gap rows are not CLI switches. Parity is measured against the
[single pinned oracle bundle](../decisions/0002-oracle-pinning.md), not every
historical TLC release.
