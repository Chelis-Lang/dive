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
| Explicit final temporal mode | Deterministic short selection | TLC-exact selection remains a separate unfinished path |

Named opt-ins can produce `tlc+overrides`. For example, final/short temporal
checking and retained checkpoint counters must not claim pure TLC semantics.
Check the resolved profile and options rather than inferring compatibility from
one command-line flag. Host-access grants remain the approved exception: compat
never silently authorizes host I/O; unsupported operators refuse.

Always record the reported seed and fingerprint settings for replay. A seeded
mode still has to prove its draw order; the independent validator refuses random
traces whose alignment it cannot establish. Current DFID TLC iterations have
additional seed qualifications described in [DFID](dfid.md).

[The maintained differences log](../tlc-differences.md) is the authority for
minimal examples, measurements, approved defaults, matching settings and
implementation status. Planned rows there are not implemented CLI switches.
The gating reference is the [single pinned oracle bundle](../decisions/0002-oracle-pinning.md),
not every historical TLC release.

Explicit `--lncheck final --lasso tlc` selects the bounded final-only schema-2
TLC mode; ordinary `--compat=tlc` temporal scheduling remains refused. The
separate `dive-final-tlc` harness requires a clean final-schedule golden and
fresh independent witness replay. It preserves configured property identities
and checks that scalar/array names, state rows and ending markers agree with
the actual replay document. See `docs/tlc-final-api.md`; the first unchanged
full small selection has 140 named refusals and no general corpus credit.
