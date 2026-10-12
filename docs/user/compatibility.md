# Compatibility and deliberate differences

`--compat=tlc` selects the TLC profile for implemented counterparts. The CLI also
reads `DIVE_COMPAT=tlc`; Rust requests expose their own explicit compatibility
fields, and low-level checking uses `CheckOptions::tlc(seed, fp)`.
Unsupported features still refuse. The flag is not a declaration of complete
TLC coverage.

| Behavior | Dive default | TLC counterpart |
| --- | --- | --- |
| Integers | Unbounded | 32-bit range with overflow errors |
| CHOOSE/value exploration | Canonical order | TLC's encounter-dependent ordering |
| Structural comparisons | Approved TLA+ comparisons | TLC's comparison/error behavior |
| Safety workers | Automatic parallel width | One by default; an explicit width is reported |
| Fingerprints | Native combination | TLC polynomial scheme, selectable with `--fp` |
| Explicit bounded final temporal mode | Deterministic short selection | Same short selection, labeled `tlc+overrides`; TLC's exact lasso is not reproduced (`--lasso tlc` is a usage error) |
| Ordinary periodic temporal scheduling | Pending | Pending; final mode does not implement periodic scheduling |

Named opt-ins can produce `tlc+overrides`. For example, final/short temporal
checking and retained checkpoint counters must not claim pure TLC semantics.
Check the resolved profile and options rather than inferring compatibility from
one command-line flag. Host-access grants remain the approved exception: compat
never silently authorizes host I/O; unsupported operators refuse. The planned
category-grant CLI is not exposed in this checkout; host environment reads,
file operations and process execution remain named refusals.

Always record the reported seed and fingerprint settings for replay. A seeded
mode still has to prove its draw order; the independent validator refuses random
traces whose alignment it cannot establish. Current DFID TLC iterations have
additional seed qualifications described in [DFID](dfid.md).

[The maintained differences log](../tlc-differences.md) is the authority for
minimal examples, measurements, approved defaults, matching settings and
implementation status. Planned rows there are not implemented CLI switches.
The gating reference is the [single pinned oracle bundle](../decisions/0002-oracle-pinning.md),
not every historical TLC release.

`--lncheck final --lasso short` under `--compat=tlc` checks temporal properties
with TLC's value semantics and a short, independently validated lasso. TLC's
exact lasso choice is not a parity obligation (ADR 0082), so the former
`--lasso tlc` mode and its `dive-final-tlc` harness mode were removed on
2026-10-12. Ordinary `--compat=tlc` temporal scheduling remains refused.
