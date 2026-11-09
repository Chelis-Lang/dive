# Compatibility and deliberate differences

`--compat=tlc` selects the existing TLC-profile settings for integer bounds,
comparison and exploration order, fingerprints, seed handling and run defaults.
The CLI also reads `DIVE_COMPAT=tlc`; Rust requests expose explicit compatibility
fields, and low-level checking uses `CheckOptions::tlc(fp_index, seed)`.

Core compatibility is nominal under [ADR 0153](../decisions/0153-core-is-the-product.md):
known gaps are documented, and unsupported features refuse by name. Exact TLC
fidelity is not promised. Core is the default engine, so
`dive check Spec.tla --compat=tlc` runs on Core.
See the [compatibility gaps and their reproducers](../tlc-differences.md#compat-gaps).

| Behavior | Dive default | TLC counterpart |
| --- | --- | --- |
| Integers | Unbounded | 32-bit range with overflow errors |
| CHOOSE/value exploration | Canonical order | TLC's encounter-dependent ordering |
| Structural comparisons | Approved TLA+ comparisons | TLC's comparison/error behavior |
| Safety workers | Automatic parallel width | One by default; an explicit width is reported |
| Fingerprints | Native combination | TLC polynomial scheme, selectable with `--fp` |
| Symmetry with genuine temporal properties | Whole-run unreduced checking (D-005) | `--symmetry-liveness=tlc` requests quotient semantics, currently refused; explicit `unreduced` under compat is `tlc+overrides` |
| Explicit bounded final temporal mode | Deterministic short selection | Same short selection, labeled `tlc+overrides`; TLC's exact lasso is not reproduced (`--lasso tlc` is a usage error) |
| Ordinary temporal checking | Deterministic completed-level boundaries on admitted routes | Bounded final analysis on admitted routes; TLC's wall-clock schedule is not reproduced |
| Wall-clock budget (`--time-limit`) | None unless set; a set limit ends in `stopped`, exit 5 | None, as TLC has none (D-026) |
| Memory budget (`--max-memory`) | Half of physical memory, then `stopped`, exit 5 | Same ceiling; TLC itself dies with `SYSTEM_OUT_OF_MEMORY` |

Named opt-ins can produce `tlc+overrides`. For example, final/short temporal
checking and retained checkpoint counters must not claim pure TLC semantics.
Check the resolved profile and options rather than inferring compatibility from
one command-line flag. Host-access grants remain the approved exception: compat
never silently authorizes host I/O. `check --allow-io=env,files,exec` currently
supports bounded source-binding environment and TXT/UTF-8 reads through
Reference, with explicit recorded assumptions and independent offline replay.
Runtime reads, writes, processes and unproved TLC environment-name ordering
remain refusals. See [host inputs](modules.md#host-inputs-and-explicit-grants).

Action composition (`A \cdot B`) uses `--action-composition=tla` by default,
counting only composed successors. Core supports pure enumerable first stages
that assign every variable, followed by a deterministic tail containing one
assignment `v' = e` per variable with an unprimed `e`, optionally inside
conjunctions and `LET`. The tail reads the intermediate state; the reported edge connects the
original state to the final state. Duplicate final candidates still count.
Incomplete intermediates, effects, nested or recursive composition and
unproved tails give named refusals. Generation with pending conjuncts or
preassigned variables also refuses; predicates on complete steps use the same
composition relation.

```sh
dive check crates/dive-api/tests/fixtures/core-composition/specs/Reduction/Reduction.tla \
  --config crates/dive-api/tests/fixtures/core-composition/specs/Reduction/Reduction.cfg \
  --workers 1 --seed 0 --fp 0
```

This example succeeds with 3 distinct states, 7 generated states and depth 2.
Pinned TLC with its experimental composition switch counts 13 generated states,
including six intermediate states. `--compat=tlc` turns composition off; add
`--action-composition=tla` to use the same mathematical slice with the profile's
other switches. Core continues to refuse `--action-composition=tlc` and its
`-Dtlc2.tool.impl.Tool.cdot=true` spelling, which request TLC's intermediate
accounting. An explicit composition option takes precedence over the profile
regardless of flag order; `options.action_composition` reports the resolved
setting. See [D-022](../tlc-differences.md#d-022-action-composition-works-by-default-with-its-tla-meaning).

Always record the reported seed and fingerprint settings for replay. A seeded
mode still has to prove its draw order; the independent validator refuses random
traces whose alignment it cannot establish. Current DFID TLC iterations have
additional seed qualifications described in [DFID](dfid.md).

[The maintained differences log](../tlc-differences.md) is the authority for
minimal examples, measurements, approved defaults, matching settings and
implementation status. Planned rows there are not implemented CLI switches.
The gating reference is the [single pinned oracle bundle](../decisions/0002-oracle-pinning.md),
not every historical TLC release.

Eligible ordinary `--compat=tlc` requests check temporal properties after
exhaustion with TLC's value semantics and a short, independently validated
lasso; the report records the actual final/short mode and retains profile
`tlc`. TLC's exact lasso choice is not a parity obligation (ADR 0082), so there
is no `--lasso tlc` mode. Core does not implement explicit
`--lncheck final --lasso short`; with temporal properties it refuses by name.
Ordinary default-profile requests use the same analyzer at deterministic completed-level boundaries (D-014). See
[temporal checking](temporal.md#ordinary-checking-in-the-tlc-profile)
for fp0/default resident-memory eligibility and explicit limits. Excluded
temporal routes retain named refusals; TLC's wall-clock periodic trigger is not
emulated.
