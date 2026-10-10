# Pure safety symmetry conformance, 2026-10-10

This is the staged #104 safety increment, not complete symmetry parity. The
eleven manifest-backed symmetry targets in ADR 0019 all agree with the pinned
single-worker TLC goldens in the four-worker run. The file-name filters also
select the separate PaxosHowToWin Voting model, so both raw reports contain
twelve models. That additional selection is retained rather than dropped.

| Run | Agree | Timeout | Disagree / unsupported / pending validation |
|---|---:|---:|---:|
| Initial one worker | 11 | 1 | 0 |
| Four workers | 12 | 0 | 0 |
| Default profile, two selected models at four workers | 2 | 0 | 0 |

The initial FastPaxos run hit its 600-second limit. Its report remains unchanged
in `symmetry-safety-initial-w1.json`; no count was inferred from that timeout.
The four-worker report is `symmetry-safety-w4.json`. Both are byte-for-byte
harness reports, recording implementation hashes, oracle and corpus pins, flags,
machine details and timing. The second finite-corpus run includes the candidate
row reuse and unchanged-child identity optimization. Final source regression
tests also cover TLC's range-before-domain visitation and the named refusal for
infinite sets, after these captured corpus runs. No performance claim is made:
the machine had other compilation/test activity.

`symmetry-safety-default-w4.json` preserves the additional default-profile
CheckpointCoordination failure and Paxos Voting checks, both fully agreeing.
Its implementation SHA-256 matched the retained release executable before
the current-main integration rebuild. This report covers the original delivery
source, not the later temporal execution-evidence changes; integration gates
are run separately. Reproduce it with `--impl-profile dive` and these filters:
`**/MCCheckpointCoordinationFailure.cfg` and `specifications/Paxos/MCVoting.cfg`.

| Model | Four-worker distinct | Generated | Depth |
|---|---:|---:|---:|
| `CheckpointCoordination/MCCheckpointCoordinationFailure.cfg` | 88,152 | 1,302,999 | 12 |
| `FlashProtocol/MCFlashWithMutex.cfg` | 2,024 | 7,552 | 21 |
| `GermanProtocol/MCGermanCMPWithMutex.cfg` | 1,107 | 3,281 | 19 |
| `GermanProtocol/MCGermanControl.cfg` | 735 | 1,946 | 19 |
| `GermanProtocol/MCGermanData.cfg` | 852 | 2,493 | 19 |
| `MultiPaxos-SMR/MultiPaxos_MC_small.cfg` | 343,796 | 736,012 | 28 |
| `Paxos/MCPaxos.cfg` | 25 | 82 | 9 |
| `Paxos/MCVoting.cfg` | 77 | 451 | 11 |
| `PaxosHowToWinATuringAward/MCVoting.cfg` | 6,752 | 41,017 | 16 |
| `SimplifiedFastPaxos/FastPaxos.cfg` | 25,617 | 320,053 | 22 |
| `SimplifiedFastPaxos/Paxos.cfg` | 1,207 | 13,290 | 22 |
| `braf/BufferedRandomAccessFile.cfg` | 3,316 | 128,581 | 10 |

The CheckpointCoordination failure is independently trace-validated before the
harness classifies it as agreement. Dedicated clean pinned probes additionally
cover both stores and profiles, worker widths 1/4/8 and chaos scheduling,
canonicalization before VIEW, exact startup warnings, malformed declarations,
concrete counterexamples and rejection of broken trace controls.

The original delivery passed the complete workspace gate (591 tests) and both Python
import/campaign checks. The required PR gate now includes the original Paxos
Voting symmetry configuration without removing any case: all 21 CM1 models
agree at 1/4/8 workers, in a combined 23-second local gate run. All 16 generated
ValidationTestSuite smoke models also agree at those widths.

Integration onto main `bb3def5` (including the temporal execution-evidence
correction) passes the complete workspace gate with 600 tests, both Python
checks, the pinned source-surface inventory check, and rebuilt CM1/validation
smoke gates at 1/4/8 workers. The historical study reports above retain their
original binary identities and findings.

Reproduce after fetching the pinned oracles and Examples corpus into an isolated
`DIVE_CACHE_DIR`:

```sh
cargo build --release --locked -p dive-cli -p dive-conformance
symmetry_filters=()
for model in MCCheckpointCoordinationFailure MCFlashWithMutex \
  MCGermanCMPWithMutex MCGermanControl MCGermanData MultiPaxos_MC_small \
  MCPaxos MCVoting FastPaxos Paxos BufferedRandomAccessFile; do
  symmetry_filters+=(--filter "**/$model.cfg")
done
target/release/dive-conformance run --impl dive --impl-profile tlc \
  --tier all --corpus examples --no-sany --no-summary --workers 4 \
  --timeout 600 --jobs 1 --report-dir symmetry-study "${symmetry_filters[@]}"
cargo test --locked -p dive-check --test symmetry
cargo test --locked -p dive-api --test symmetry_trace
```

Remaining partial contracts stay named refusals: deferred/lazy/infinite value
permutation, unsupported declaration shapes, unverified startup exception
wrapping, randomness under symmetry and symmetry with liveness. The opt-in
symmetry audit is still planned. The startup fixtures and ADR 0019 enumerate
these gaps; the feature ledger must retain partial status.
