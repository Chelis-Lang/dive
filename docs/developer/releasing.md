# Releasing dive

A release is a version tag on a qualified commit of `main`. The tag drives two
workflows: `release.yml` builds the binary archives and creates the GitHub
Release, then `publish-crates.yml` publishes the crates to crates.io. The bar a
release must meet is in [RELEASE.md](../RELEASE.md); publication of the crates
is recorded in [ADR 0157](../decisions/0157-crates-io-publication.md).

## Published crates

Eighteen crates are published, all at the workspace version: `dive-cli` (the
`dive` and `tlc` binaries), `dive-api` and the library crates it depends on.
Each opts in with `publish = ["crates-io"]`; `dive-conformance`, `dive-bench`,
`xtask` and `fuzz` keep the workspace default `publish = false`.

Package contents are set by each crate's `include` list: `src/`, the README,
the `LICENSE` copy and any data compiled into the crate (`dive-sema`'s
`stdlib/` and `bundled/`, `dive-cli`'s `build.rs` and `build/`). Tests,
examples and benchmarks are not shipped. Internal dev-dependencies are written
as plain paths so packaging drops them. A new published crate needs the same
manifest fields, a short `README.md` and a byte-identical copy of the
repository `LICENSE`, which the publisher checks.

## Procedure

1. **Bump the version** in one PR: `[workspace.package] version`, every
   `version = "=X.Y.Z"` pin in `[workspace.dependencies]`, `Cargo.lock`
   (`cargo update --workspace`), and a curated `## X.Y.Z` section in
   `CHANGELOG.md` (the release notes are extracted from it).
2. **Dry-run** on the merged commit, with a clean worktree and Python 3.11 or
   later:

   ```sh
   python3 scripts/test-publish-crates.py
   python3 scripts/publish-crates.py --dry-run --version X.Y.Z
   ```

   The publisher refuses a dirty worktree, packages all crates together with
   `cargo package --locked`, builds every package against the other packages,
   and reports which versions are not yet on crates.io. It reads no
   credential. A manual dispatch of `release.yml` runs the same dry run
   alongside the archive build.
3. **Tag** the qualified commit and push the tag (`git tag vX.Y.Z <sha>`,
   `git push origin vX.Y.Z`). `release.yml` checks the tag against the
   version, builds the Linux archives, repeats the crate dry run and creates
   the GitHub Release.
4. **Publication.** After creating the release, `release.yml` dispatches
   `publish-crates.yml` with the tag and commit. That workflow requires the
   release to be published and the tag to resolve to the same commit, checks
   out that commit and runs `scripts/publish-crates.py --version X.Y.Z` with
   the `CARGO_REGISTRY_TOKEN` repository secret. Only this job receives the
   secret. Versions already on crates.io with an identical archive checksum
   are skipped, so the workflow is safe to rerun:

   ```sh
   gh workflow run publish-crates.yml -f tag=vX.Y.Z -f commit=FULL_COMMIT_SHA
   ```

   A release created by hand in the GitHub UI starts the workflow through its
   `release: published` trigger instead.

## Initial publication

The first publication of each crate name is done by the owner from a local
clean checkout of the tagged commit, with a crates.io API token in a file
(never in arguments, the environment of other commands or the repository):

```sh
python3 scripts/publish-crates.py --version 0.1.0 --token-file PATH
```

The publishing account needs a verified email address. crates.io limits how
quickly new crate names can be created, so the first run may stop part way
with a rate-limit error; rerun the same command from the same commit later and
it resumes after the versions already published. Once every name exists,
store a token scoped to publish-update for these crates as the
`CARGO_REGISTRY_TOKEN` repository secret; later releases publish through the
workflow.

## Verification

After publication, install the exact version with an empty Cargo home, outside
the checkout:

```sh
export CARGO_HOME="$(mktemp -d)"
cargo install dive-cli --version X.Y.Z --locked
"$CARGO_HOME/bin/dive" --version
```

`--version` prints `dive X.Y.Z (<commit>)`, where `<commit>` is the first 12
hex digits of the tagged commit (from the `.cargo_vcs_info.json` Cargo writes
into the package). A build from a Git checkout prints `git describe` output
instead. Then check a small specification with `dive check` and `tlc`, and
build a scratch crate that depends on `dive-api = "=X.Y.Z"`.

A broken published version cannot be replaced: yank it
(`cargo yank --version X.Y.Z <crate>`) and release a new version of every
crate.
