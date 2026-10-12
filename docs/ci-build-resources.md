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
