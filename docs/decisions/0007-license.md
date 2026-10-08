# 0007: License: MIT

- **Status:** accepted (supersedes the earlier "license deferred" decision)
- **Date:** 2026-10-08

## Context

The charter requires confirming D.I.V.E.'s license with the owner before the first public release. Org repos vary: MIT for the older shells, `Apache-2.0 OR MIT` for barnacle, and unlicensed for buoy. The canonical tlaplus tools are MIT.

## Decision

dive is licensed **MIT**, as confirmed by the owner on 2026-10-08.
- The repo has a `LICENSE` file.
- `license = "MIT"` is set in `[workspace.package]` and inherited by every crate.
- Crates stay `publish = false` until a release decision is made.
- Code ported from the MIT-licensed tlaplus tools carries attribution, recorded in `docs/provenance.md`.

## Consequences

- dive is licence-compatible with the canonical tools and with the dependency allowlist in `deny.toml`.
- Publishing to crates.io is a separate, later decision.
