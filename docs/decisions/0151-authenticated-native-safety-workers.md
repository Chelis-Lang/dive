# 0151: Authenticated native pure-safety workers

- **Status:** accepted
- **Date:** 2026-11-04

## Context

ADR 0141 binds pure Core continuation to source/configuration, executable and
semantic identity while preserving actual owned values. Native distribution
must evaluate real source units on independent processes, retain deterministic
ordered commit and never infer completion from a lost partial result. A second
generator, coordinator shadow evaluation or growing Store image per successor
would undermine this contract. Temporal paging is a separate obligation.

## Decision

Add an explicit pure ordinary safety route in `dive-explore`, exposed by
`dive check --distributed-worker`, `dive worker`, `CheckRequest::distributed`
and `prepare_core_worker`. Distribution stays off by default in both profiles;
`None` is the TLC-equivalent setting. Both roles independently load and compile
the same model. No work message selects source, executable, paths or host grants.

Use TCP with a provisioned 32-byte group secret and HMAC-SHA256 through
RustCrypto's public `hmac` 0.13 API (`Mac::verify_slice`), the existing `sha2`
0.11 implementation, and `getrandom` 0.4 `fill` for operating-system nonces.
These are normal workspace dependencies of `dive-explore`; serde/serde_json
encode typed metadata. No internal crate edge changes, Java serialization or
dependency from independent `dive-trace` to the checker is introduced.
HMAC is the established primitive, not a new cryptographic construction:
[RFC 2104](https://www.rfc-editor.org/rfc/rfc2104),
[RustCrypto API](https://docs.rs/hmac/0.13.0/hmac/),
[OS random API](https://docs.rs/getrandom/0.4.3/getrandom/).

The fixed handshake authenticates protocol version, ADR 0141 source/config and
executable identity, the full evaluator/symmetry policy, Core switches and exact
compiled startup. It also binds fresh run and client/server nonces and an
epoch/work/attempt ticket. Authenticated worker UUID/PID are process facts, not
remote attestation. Possession of the group key identifies an explicitly trusted
participant; this is not Byzantine verification or individual key infrastructure.
The acknowledgement signs the complete hello, status and server identity. Every
frame MAC includes role, full session binding, kind, directional sequence and
length plus payload. The receiver verifies the MAC before interpreting payloads.
Duplicate earlier sequences are ignored under a bounded admission; future frames
and other session bindings reject. Replaying a hello cannot replay work under a
fresh server nonce. The transport provides authentication/integrity, not encryption;
operators use private networks or independently provided encrypted tunnels.

At most one source per endpoint participates in a bounded batch. Workers use the
existing `parallel::expand_one`/`WorkerSink`, with a read-only membership RPC to
the coordinator's real Seen authority. Seen is frozen during a batch; ordered
commit precedes the next batch. Exact lookup compares native canonical key bytes
only within the actual fingerprint bucket. No growing Seen snapshot or global
shadow Next evaluation is sent or run. The coordinator alone commits source and
successor order, suppresses duplicates, assigns IDs, updates logical counts,
selects first failures and reconstructs selected words. Init and selected-word/
diagnostic replay remain local through the existing engines. Network completion
order never determines logical commit order.

Bounded authenticated reply bytes retain their matching request authority until
source-ordered consumption. Only then are values imported into the coordinator's
Store/history. Later unused failed work is reported in `failed_units` and cannot
replace an earlier selected model error. A failed unit required by continued
search becomes a certification failure, so incomplete search cannot yield Holds.

An owned child-first DAG transports original and canonical rows separately.
Names must match the fixed sealed startup table. Local handles, transient codec
tokens and Reference Value projections are not authority. Startup references are
ordinals into a separately identity-checked local startup index; input references
belong to the matching request. Explicit constructor-origin indexes preserve
raw/normalized companion nodes and aliases. Import cannot replace an already
issued constructor phase with another payload. The worker restores only its
fixed startup image per unit, then releases that unit's Store, Arena and memo
roots. The coordinator never sends a growing full Store image per successor.

I/O loss retries the complete uncommitted source with a new attempt/session on
the configured endpoint ring. Dropped prefixes are discarded. Only complete,
authenticated and structurally admitted results participate in ordered commit.
Integrity/identity/admission failures do not retry into a successful verdict.
The initial route requires 2..64 distinct endpoints and at least two authenticated
worker instances, explicit seed/fingerprint, resident fp or exact Seen and pure
safety roles without VIEW/symmetry. Frontier/predecessor spill remain coordinator
operations. Temporal checking, effects, new runtime names, sampling, coverage,
disk Seen, candidate spill, Store reclamation/remapping and checkpoint/pause/recovery
combinations remain unsupported. Coordinator and public worker preparation reject
Store reclamation and TLC logical coverage before source/configuration or key I/O.

Private admission bounds are 64 MiB/frame, 32 MiB/owned DAG or compiled startup,
128 DAG nesting levels, 1 MiB/canonical membership key, 16,384 emissions/source,
64 duplicate frames/read and the existing startup image's 65,536-name bound.
Timeouts are nonzero through 60 seconds; retry count is at most eight. Unit and
socket deadlines bound retries and cancellation latency. Limits do not silently
constrain ordinary local checking. No incomplete unit proves Holds.

## Consequences

This is a useful partial native distribution family, with actual independently
initialized CLI and Rust worker processes. Live coordinator loss/durable epoch
recovery, temporal distribution, dynamic membership and broader storage/effect
combinations remain open. A worker restart at the same configured endpoint can
continue uncommitted work; a coordinator restart cannot recover this run.

Protocol admission does not bound total RSS or establish speedup. Coordinator
Seen/accepted Store/history and imported speculative batch roots, frontier,
predecessors, names, startup/plan roots, evaluator Arenas/caches, metadata/DAG/key
scratch, socket buffers, allocator overhead and remote process memory remain
separate owners. Existing `max_memory` observes the coordinator process only.
Authentication trusts correct worker execution; independent trace validation
continues to validate actual reported words and falsified ending controls.
