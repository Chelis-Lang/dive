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
| Wall-clock budget (`--time-limit`) | 30 minutes (provisional), then `stopped`, exit 5 | None, as TLC has none (D-026) |
| Memory budget (`--max-memory`) | Half of physical memory, then `stopped`, exit 5 | Same ceiling; TLC itself dies with `SYSTEM_OUT_OF_MEMORY` |

Named opt-ins can produce `tlc+overrides`. For example, final/short temporal
checking and retained checkpoint counters must not claim pure TLC semantics.
Check the resolved profile and options rather than inferring compatibility from
one command-line flag. Host-access grants remain the approved exception: compat
never silently authorizes host I/O; unsupported operators refuse. The planned
category-grant CLI is not exposed in this checkout; host environment reads,
file operations and process execution remain named refusals.

Action composition (`A \cdot B`) uses `--action-composition=tla` by default,
counting only composed successors. `--compat=tlc` turns it off; an explicit
`--action-composition=tlc` or `-Dtlc2.tool.impl.Tool.cdot=true` enables TLC's
experimental behavior, including intermediate generated counts and sequential
BFS checking. DFID and simulation count only emitted successors in either
enabled mode. An explicit composition option takes precedence over the profile,
regardless of flag order, and `options.action_composition` reports the resolved
setting. Default mode refuses the enclosing conjunctions TLC's algorithm
distorts. Independent trace replay currently refuses enabled composition when
it requires intermediate-state enumeration. See [D-022](../tlc-differences.md#d-022-action-composition-works-by-default-with-its-tla-meaning).

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
2026-10-12. Eligible ordinary TLC-profile requests select the same bounded final
analyzer without extra policy flags; the report records the actual final/short
mode and retains profile `tlc`. Ordinary default-profile requests use the same
analyzer at deterministic completed-level boundaries (D-014). See
[temporal checking](temporal.md#ordinary-checking-in-the-tlc-profile)
for fp0/default resident-memory eligibility and explicit limits. Excluded
temporal routes retain named refusals; TLC's wall-clock periodic trigger is not
emulated.
