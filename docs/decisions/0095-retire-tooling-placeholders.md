# 0095: Retire post-v1 compiler/LSP placeholders

- **Status:** accepted
- **Date:** 2026-10-13

## Context

The approved [rescue plan](https://github.com/Chelis-Lang/dive/issues/241#issuecomment-6128600987)
explicitly removes the `dive-compile` and `dive-lsp` stubs and returns them after
v1 as fresh crates. Both crates contain only crate documentation and
`forbid(unsafe_code)`, with empty dependency lists and no runtime APIs or tests.
They remain workspace members through the `crates/*` wildcard despite having
no production callers.

## Decision

Remove both placeholder crates, their workspace dependency declarations and
lockfile package records. Remove their nodes from the permitted internal
dependency graph in `xtask/src/layering.rs`, including the unused permissions
for `dive-api` to depend on `dive-compile` and for `dive-cli` to depend on
`dive-lsp`. All remaining dependency permissions and third-party lockfile
packages stay unchanged.

The charter's language-server and native-compilation responsibilities remain
on the [post-v1 roadmap](../ROADMAP.md#after-v01). Future implementations
return as fresh crates with an explicit dependency-graph decision and the
required conformance checks.

## Consequences

Workspace checks no longer compile two empty packages. This removes no
implemented feature, runtime test or conformance entry and grants no parity
credit. Future compiler and language-server work cannot rely on placeholder
package membership or unused dependency permissions.
