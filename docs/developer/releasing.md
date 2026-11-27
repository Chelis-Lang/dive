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
   Update the [docs surfaces before the tag](#before-the-tag) in the same
   PR or one just after it.
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
3. **Check the docs surfaces** on the commit to be tagged. This must print
   `clean`:

   ```sh
   python3 -I scripts/check-release-docs.py --version X.Y.Z --previous A.B.C
   ```

   Before the paired Sail release is tagged, add `--sail PATH/TO/sail
   --sail-version S.T.U` to check the Sail files that crates.io freezes: its
   CHANGELOG section, README, install guide, crate READMEs, Action default
   and the dive release its pages name. No Sail tag commit is needed yet.

4. **Tag** the qualified commit and push the tag (`git tag vX.Y.Z <sha>`,
   `git push origin vX.Y.Z`). `release.yml` checks the tag against the
   version, builds the Linux archives, repeats the crate dry run and creates
   the GitHub Release.
5. **Publication.** After creating the release, `release.yml` dispatches
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
6. **Verify** the published crates (see [Verification](#verification)).
7. **Update the docs surfaces after publication** (see
   [After the tag and publication](#after-the-tag-and-publication)).

## Docs surfaces

Every release updates every docs surface. dive and Sail release together, so
run this checklist alongside the docs step of Sail's playbook
([`docs/releasing.md`](https://github.com/Chelis-Lang/sail/blob/main/docs/releasing.md)
in Chelis-Lang/sail). Anything that quotes release-candidate measurements is
written once the qualification has posted its numbers in the release issue.

### Before the tag

These ship from the tagged commit, so they are fixed before it is tagged.

- [ ] `README.md`: install command, headline numbers and what the release
      includes.
- [ ] `crates/*/README.md`: crates.io keeps the README of each published
      version and it cannot be edited afterwards.
- [ ] `CHANGELOG.md`: a curated `## X.Y.Z` section with measured numbers and no
      placeholder notes. `release.yml` copies it into the GitHub Release.
- [ ] `docs/user/install.md` (archive URLs and names, `--version` output) and
      `docs/user/rust-api.md` (the `dive-api` dependency line).
- [ ] `docs/tlc-differences.md`: every divergence registered for the release.
- [ ] `docs/developer/release-performance-results.md`: the results page names
      the release and carries its measurements.
- [ ] `docs/scoreboard.md`: the gate rows show the release-candidate results.
- [ ] `skills/dive/SKILL.md`: install line and commands match the release.
- [ ] No user-facing page names a feature the release leaves out, other than
      its preview page.

The checker covers the mechanical part of this list: the CHANGELOG section,
stale versions in install commands, dependency lines and download URLs, and
unshipped feature names. Its file and pattern tables are at the top of
`scripts/check-release-docs.py`; its tests are
`scripts/test-check-release-docs.py`.

### After the tag and publication

These live outside this repository and point at the published release. Each
outward-facing change below (the website deploy, a release post, the GitHub
repository settings) needs the owner's approval before it goes out.

- [ ] **Dive book on jeffreykent.dev.** In `jeffreykentjr/mesite`, set the
      `dive` entry of `src/data/docs.json` to the tag commit and the SHA-256 of
      its archive:

      ```sh
      git rev-parse vX.Y.Z^{commit}
      curl -sL https://codeload.github.com/Chelis-Lang/dive/tar.gz/FULL_COMMIT_SHA | sha256sum
      ```

      Follow mesite's `docs/publishing.md` for its checks, review and merge.
      The paired Sail release moves the `sail` entry of the same file and the
      cproof.ai pin in Chelis-Lang/website; Sail's playbook owns those.
- [ ] **Dive pages on jeffreykent.dev.** Update `src/content/guides/dive.md` and
      `src/content/guides/dive/*.md` in mesite (source tag, revision and
      numbers), and redraw the share cards if a title or description changes.
- [ ] **Release post.** The owner decides whether the release gets a post in
      mesite's `src/content/blog/` and writes or approves its wording.
- [ ] **GitHub repository metadata.** Set the homepage to the hosted book and
      the topics to the crates.io keywords plus `rust`:

      ```sh
      gh repo edit Chelis-Lang/dive --homepage https://jeffreykent.dev/docs/dive/ \
        --add-topic tla --add-topic tlaplus --add-topic model-checker \
        --add-topic formal-verification --add-topic pluscal --add-topic rust
      ```

- [ ] **Internal consumers.** Open an issue or PR in `jeffreykentjr/tire`
      (README install lines and the CI install of dive) and `Chelis-Lang/oyster`
      (README, docs and the Sail pin in `Cargo.toml`) asking them to move to
      `cargo install dive-cli --version X.Y.Z --locked` and the paired Sail
      release.
- [ ] **Recheck** with every checkout given, from this repository's root:

      ```sh
      python3 -I scripts/check-release-docs.py --version X.Y.Z --previous A.B.C \
        --tag-commit FULL_COMMIT_SHA --mesite PATH/TO/mesite \
        --sail PATH/TO/sail --sail-version S.T.U --sail-tag-commit SAIL_COMMIT_SHA \
        --website PATH/TO/website
      ```

      With the site checkouts on their bumped branches this also confirms the
      book pins, and Sail's README, install guide, crate READMEs and Action
      default name the Sail release.

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
