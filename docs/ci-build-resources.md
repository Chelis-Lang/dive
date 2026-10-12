# CI build resources

The required Workspace gate combines Workspace checks with all sixteen test
partitions. Every prerequisite must succeed. The complete inventory still builds
and lists the real workspace; retained JSON cannot replace that build.

Workspace checks uses two Cargo jobs and an explicit checks target/cache namespace.
Clippy uses a temporary directory owned by the current workflow run and attempt.
After Clippy succeeds, the job removes only that completed temporary directory
before compiling the test inventory. Invalid IDs, symlinks and non-directory
paths are rejected. Other targets and failed Clippy outputs are untouched by this
cleanup step. The job records disk and memory measurements around these stages.

This reduces overlapping build outputs, at the cost of less artifact reuse.
Commands, compiler profiles, test selections, assertions and time limits remain
unchanged. See the [workflow](../.github/workflows/ci.yml) and
[partition verifier](../scripts/check-test-partitions.py).

## Retained failure and qualification

Two attempts on source `6b4113aa` failed in the inventory's real nested
`cargo test --no-run` build: `cc`/`rust-lld` terminated with SIGBUS. The sixteen
test partitions and conformance succeeded separately; the aggregate correctly
rejected the failed checks prerequisite. The second attempt restored about
125 MB of cache. Those logs establish neither disk exhaustion, OOM nor cache
corruption. The resource change is a mitigation experiment, not a proved diagnosis.

The closed receiving source retains its own full local gate and runtime results.
The workflow change needs a separate full local gate and exact-head CI result.
No failed attempt is relabelled as passing and no semantic family is certified
by operational success.

## Local command inventories

A workflow's inline Python heredoc is a program supplied on stdin, not a list
of shell arguments. A local gate driver must preserve and hash the exact body,
execute it explicitly, and reject unknown inline invocations. Testing directory
cleanup locally uses a small owned temporary fixture and an unrelated sibling;
it must not remove a developer's build directories. That fixture proves deletion
scope, not the runner's available resources or a repaired linker.

Keep every original script invocation, including duplicates, and run the complete
Rust gate. Preserve command outputs, exits and producer identities. Measurements
from the actual Linux job determine whether the mitigation worked.

## Hosted Linux SDK bootstrap experiment (2026-10-12)

The retained partition 2 log for source `79e96185` restored about 89 MB of
cache, then reported only 17 MB of free disk before a linker SIGBUS. This
attempt establishes severe disk exhaustion in that job. Workspace checks
separately emitted `No space left on device` during the inventory build; its
post-failure measurement showed 8.1 MB of disk and 6.9 GiB of available memory.
These observations establish disk exhaustion in both inspected jobs. They do
not establish cache corruption, the cause of every other failed partition, or
the cause of the earlier `6b4113aa` failures. All sixteen test partitions failed
in the new attempt; conformance passed. Keep the individual logs and receipts.

Before Rust/tool/cache installation, Workspace checks and each Workspace test
partition remove only the unused preinstalled Android, .NET and GHC SDK
directories selected by [the helper](../scripts/ci-reclaim-hosted-sdk-space.sh).
The workflow selects GitHub-hosted Linux runners, and the helper independently
requires that explicit context. It validates every existing selected path as an
unredirected directory on the root filesystem, refuses symlink or mounted roots,
and uses one-filesystem deletion. Missing SDK directories are harmless. Cleanup
errors fail the job; disk usage is recorded before cleanup and on every exit.

The helper does not select the hosted tool cache, Rust/Cargo, Java/JRE, Node,
Python, dependency caches, workspace files or retained evidence for deletion.
The original 73 Python script invocations and exact inline Python body remain
unchanged, as do the sixteen partitions, all Rust commands, profiles, assertions,
cache settings and timeouts. This is ephemeral job resource bootstrap, not a
semantic change or a replacement for any required check.

Actual hosted-runner execution and the next exact-head CI result are still
pending. Static source review and shell syntax validation are not proof that
this mitigation frees enough space or repairs the linker. Never run this helper
as local stale-build cleanup. The next source qualification must retain the
helper's exact bytes in addition to the unchanged 74 Python inventory records
(73 scripts and one inline program).
