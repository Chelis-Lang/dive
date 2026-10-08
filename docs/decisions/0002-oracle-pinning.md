# 0002: Oracle pinning and Java runtime

- **Status:** accepted
- **Date:** 2026-10-08

## Context

The charter calls for pinning `tla2tools.jar` by release version and SHA-256, and for being able to run conformance against more than one TLC release. Upstream reality on 2026-10-08:

- **v1.7.4** (2024-08-05) is the latest *stable* release. It publishes no checksum.
- **v1.8.0** is a *rolling* pre-release. The workflow `.github/workflows/main.yml` force-moves its tag and re-uploads `tla2tools.jar` on every master push, so its URL and digest change over time. The upstream Examples CI uses this rolling build, via the nightly mirror.
- **CalVer tags** such as `2026.10.06.014338` mark each master build. They are git tags only, with no release assets.
- **Java and build provenance:**
  - TLC needs Java 11+.
  - The 1.8.0 jar's manifest records Temurin 17 and the `X-Git-Revision` it was built from.
  - The development machine and CI runners have no Java installed.

## Decision

- **Primary oracle `tlc-2026.10.06.014338`:**
  - It is the 1.8.0 jar built from upstream commit `94d0c50`, downloaded unmodified.
  - It was verified against GitHub's asset digest `sha256:7beec0f0…` and the manifest's `X-Git-Revision`.
  - It is re-hosted as release `oracle-tlc-2026.10.06.014338` on `Chelis-Lang/dive`, together with the MIT licence and a NOTICE.
- **Secondary oracle `tlc-1.7.4`:** fetched from upstream, verified against our own recorded SHA-256.
- **Classpath:** the CommunityModules release jars `202610040242` are pinned the same way. Examples specs depend on them.
- **Java runtime:**
  - Eclipse Temurin **JRE 17.0.20.1+1**, pinned per platform by Adoptium's published SHA-256.
  - The harness downloads and unpacks it into `$DIVE_CACHE_DIR`.
  - Using a system `java` is an explicit opt-in (`--java`), recorded in report provenance.
- **Location of pins:** all pins live in `conformance/oracles.toml`. Every artifact is checked by SHA-256 before use, and a mismatch is fatal.
- **Provenance:** every golden and report records the oracle id and SHA-256.
- **Refresh:**
  - A new snapshot gets a new id and release. Old ids stay available.
  - Changing the default oracle requires regenerating goldens with a recorded reason and reviewing the diff.

## Consequences

- The private release asset needs a GitHub token to fetch. CI has one; locally the harness uses `gh auth token`.
- **Default choice:** goldens follow the current upstream TLC, which is also what Examples is validated against, rather than a two-year-old stable release.
- **Drift check:** keeping v1.7.4 lets us detect oracle drift between releases.
- **Refresh cadence** is an open owner question.
