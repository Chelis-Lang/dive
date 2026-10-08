# 0007: License deferred; crates unpublished

- **Status:** accepted
- **Date:** 2026-10-08

## Context

The charter requires confirming D.I.V.E.'s license with the owner before the first public release. The repo is private. Org repos vary: MIT for the older shells, `Apache-2.0 OR MIT` for barnacle, and unlicensed with `publish = false` for buoy.

## Decision

No license is declared yet, and every crate has `publish = false`. `deny.toml` ignores licences for private workspace crates.

Code ported from the MIT-licensed tlaplus tools carries attribution, recorded in `docs/provenance.md`, so any later choice remains compatible.

## Consequences

The licence is an open question for the owner (see `docs/open-questions.md`). Both MIT and `Apache-2.0 OR MIT` stay compatible with the dependency allowlist.
