# 0157: Publish the library and CLI crates to crates.io

- **Status:** accepted
- **Date:** 2026-11-17

## Context

Until 0.1.0 every crate was `publish = false`. Users installed the CLI with
`cargo install --git`, and Rust callers took a path or Git dependency on
`dive-api`. Neither gives a versioned, immutable, discoverable artifact, and a
Git dependency builds whatever the branch holds. crates.io packages also need
every dependency published, so `dive-api` (and `dive-cli`, which depends on
it) can only be published together with the crates beneath them.

The sibling project Sail already publishes its CLI and library crates with a
credential-free verification script and a release-triggered workflow; dive
follows the same procedure.

## Decision

1. Eighteen crates are published to crates.io: `dive-syntax`, `dive-value`,
   `dive-sema`, `dive-config`, `dive-store`, `dive-pluscal`, `dive-ir`,
   `dive-eval`, `dive-core`, `dive-stdlib`, `dive-explore`, `dive-model`,
   `dive-runtime`, `dive-trace`, `dive-sim`, `dive-liveness`, `dive-api` and
   `dive-cli` (binaries `dive` and `tlc`). `dive-conformance`, `dive-bench`,
   `xtask` and `fuzz` stay unpublished. The workspace default stays
   `publish = false`; each published crate opts in with
   `publish = ["crates-io"]`, and the publisher refuses any member that does
   neither.
2. **Lockstep versions.** All published crates share the workspace version,
   and every internal requirement is an exact `=VERSION` pin in
   `[workspace.dependencies]`. A release bumps them together. Mixing versions
   of dive crates is unsupported, so the exact pins make Cargo refuse it.
3. **Public contract.** The published names, the `dive` and `tlc` binaries and
   `dive-api`'s public API are a public contract under semver from 0.1.0.
   The other library crates are published because `dive-api` needs them; they
   are usable, but their APIs follow `dive-api`'s needs and may change in any
   minor release while the version is 0.x.
4. **Package contents.** Packages ship `src/`, the README, the licence and the
   data compiled into the crate (`dive-sema`'s embedded modules and their
   licence notices, `dive-cli`'s build script). Tests, examples and benchmarks
   read workspace fixtures and are not shipped. Internal dev-dependencies are
   plain paths, which Cargo drops when packaging, so dev-dependency cycles
   (for example `dive-eval` testing through `dive-model`) do not block
   publication.
5. **Procedure.** `scripts/publish-crates.py` packages and verifies all crates
   together from a clean commit before it reads any credential, skips versions
   already published with identical bytes, and publishes the rest.
   `.github/workflows/publish-crates.yml` runs it for a published GitHub
   release with the `CARGO_REGISTRY_TOKEN` secret; the procedure is in
   `docs/developer/releasing.md`.
6. A packaged `dive-cli` has no Git checkout, so `--version` names the commit
   Cargo recorded in `.cargo_vcs_info.json` when packaging it.

## Consequences

- `cargo install dive-cli --locked` and `dive-api = "0.1"` work without a
  checkout.
- A published version can only be yanked, never replaced; a fix needs a new
  version of every crate.
- Every release bumps all eighteen crates even when only one changed.
- Renaming, splitting or merging a published crate is now a decision with a
  user-visible cost; the layering table (`xtask/src/layering.rs`) still
  governs internal edges.
- crates.io limits how fast new crate names can be created, so the initial
  publication may need several resumed runs.
