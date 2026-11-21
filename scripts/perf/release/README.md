# Release performance drivers

The scripts that produced the 0.1.0 release performance records in
`conformance/reports/perf/` (D1 to D7 and S7; the data set is described in
`conformance/reports/perf/README.md`). They are kept so that the next release
can repeat the campaign and compare. They are working tools, not a product:
they are accurate to what ran, runnable in principle, and still carry some
0.1.0 constants (listed below). Nothing here runs in CI.

The method is `docs/developer/release-performance-plan.md`; the R2 procedure
is `docs/developer/performance.md#r2-measurement-procedure`. Read both first.

## Layout

| Directory | Runs on | Contents |
|---|---|---|
| `ops/` | the operator's machine | SSM helpers: run a command on a named node, copy small files in and out, presigned bucket URLs, volume and mac-host helpers |
| `node/` | each bench node, as user `bench`, from `~/tools/` | build and host facts, the D2/D3/D5/D6 driver, the chains that run measurements back to back, D4 rebalancing, the D7 mac script |
| `records/` | the operator's machine, from a dive checkout | turn pulled summaries into the compact records and the results-page tables |
| `sail/` | the Sail node, from `~/tools/` | S1 to S7: node bootstrap, Sail builds, the Sail harnesses, the S7 chains and record builder |

## Environment

The operator side needs the AWS CLI (SSO login), `jq`, `zsh`, Python 3 with
`boto3` for `presign.py`, and:

| Variable | Used by | Meaning |
|---|---|---|
| `AWS_PROFILE` | `ops/*`, `scripts/aws/*` | SSO profile for the dive account |
| `AWS_REGION` | same | defaults to `us-east-1` |
| `DIVE_PERF_STATE` | `ops/n`, `vol`, `watch`, `mac-up` | a private file of `name=instance-id` lines (`small=...`, `shard1=...`, `spot1=...`); keep it outside the repository |
| `DIVE_EVIDENCE_BUCKET`, `DIVE_PERF_PREFIX` | `ops/presign.py` | evidence bucket name and the campaign prefix, for example `release-perf/2026-11-18/` |
| `DIVE_AWS_AMI`, `DIVE_AWS_INSTANCE_PROFILE`, `DIVE_AWS_GH_TOKEN_PARAM` | `scripts/aws/bench-node.sh`, `ops/mac-up` | as documented in `bench-node.sh` |
| `DIVE_REV` | `node/chain-*.sh`, `node/d4x.sh`, `node/d7-mac.sh` | revision passed to `dive-bench --dive-revision`; defaults to the checkout's `HEAD` |
| `DIVE_REPO` | `records/d4record.py` | dive checkout holding `conformance/expected-divergences.toml`; defaults to this repository |

Never put instance ids, account numbers or addresses in a file under the
repository; they belong in `DIVE_PERF_STATE`.

## Running a campaign

The 0.1.0 campaign used five on-demand c7i.metal-24xl nodes for D1 (one for
the small class and then D5, D3, D6, D2; four for the large shards and then
D4), five spot nodes of the same type for later D4 repetitions, a laptop for
D7, and one c7i.8xlarge for S7.

### 1. Bootstrap the nodes

```sh
scripts/aws/bench-node.sh <rev> --list      # once per node; prints the instance id
echo "small=<instance-id>" >> "$DIVE_PERF_STATE"   # and shard1..shard4
scripts/perf/release/ops/vol small modify   # 16000 IOPS, 1000 MiB/s; repeat per node
```

`bench-node.sh` turns SMT and turbo off, fixes 2.4 GHz and the `performance`
governor, builds `<rev>`, and fetches the oracle and corpora; `--list` makes
its own `dive-bench` run a no-op. For spot nodes, `DRY_RUN=1
scripts/aws/bench-node.sh <rev> --list` prints the same user data, which was
passed to `aws ec2 run-instances` with `--instance-market-options
MarketType=spot`.

Copy the node tools (each file must be under about 40 KB; `push` sends it
through one SSM command):

```sh
for f in scripts/perf/release/node/*; do
  scripts/perf/release/ops/push small "$f" "/home/bench/tools/$(basename "$f")"
done
```

### 2. Build once and copy

Independent builds of one source on different nodes were not byte-identical,
so the release binary is built on one node and copied:

```sh
scripts/perf/release/ops/n small 'sudo -iu bench bash ~/tools/node-prep.sh <rev>'
# writes ~/bins/{dive,dive-bench,dive-conformance} and ~/hostinfo.txt; then upload:
URL=$(scripts/perf/release/ops/presign.py put bins/dive)
scripts/perf/release/ops/n small "curl -sS -T /home/bench/bins/dive '$URL'"
# on every other node (spot-prep.sh downloads, checks <rev>, records host facts):
scripts/perf/release/ops/n spot1 "sudo -iu bench bash ~/tools/spot-prep.sh '<dive-get-url>' '<bench-get-url>' <rev>"
```

Check `sha256sum ~/bins/dive` on every node against the first node's.

### 3. D1: classify, shard, time

The selection is the model list of the previous R2 record (the 175 keys of
`2026-11-10-r2-procedure-e3835968.json`'s `models`), one per line. The harness
carries out the whole procedure (`dive-conformance r2`, documented in
`docs/developer/performance.md`). It refuses a dirty environment, binds itself
with `numactl`, warms the page cache, and times one checker at a time. Long jobs
run as transient systemd units so they survive the SSM command:

```sh
N=scripts/perf/release/ops/n
R2='env -i PATH=/usr/bin:/bin HOME=/home/bench DIVE_CACHE_DIR=/home/bench/.cache/dive /home/bench/bins/dive-conformance --repo-root /home/bench/dive r2'
$N small "systemd-run --unit=r2-classify --uid=bench --working-directory=/home/bench $R2 classify --filter-file /home/bench/tools/selection.txt --out /home/bench/r2"
# copy ~/r2/r2-classes.json to every shard node's ~/r2/, then:
$N small  "systemd-run --unit=r2-small --uid=bench ... $R2 small --classes /home/bench/r2/r2-classes.json --dive-bin /home/bench/bins/dive --out /home/bench/r2"
$N shard1 "systemd-run --unit=r2-large --uid=bench ... $R2 large --classes /home/bench/r2/r2-classes.json --shard 1/4 --dive-bin /home/bench/bins/dive --out /home/bench/r2"
```

`--shard K/N` splits the large class by longest processing time on the golden
TLC times, so the split is fixed before any timing. The shard's model list,
which `node/chain-shard.sh` reads as `~/tools/shard.txt`, is
`jq -r '.models | keys[]' ~/r2/r2-large-shardKofN.json`.

### 4. Chains

Each chain waits for the node's D1 unit to finish and then runs the next
measurements back to back, one process at a time, logging to `~/chain.log`:

- `node/chain-small.sh` (small node): D5, D3, D6 through `dmeasure.py`, then the
  D2 benchmark set through `dive-bench --profile both --workers 1,8 --reps 5
  --tlc --xmx 3g`, then touches `~/m/CHAIN_DONE`.
- `node/chain-shard.sh` (each shard node): writes a `dive-bench` set file from
  `~/tools/shard.txt` (less `MCCheckpointCoordinationFailure`, whose golden is a
  violation) and runs D4: `--profile default --workers 1,4,8 --reps 3
  --timeout 3600 --xmx 24g`.
- `node/d2-chain.sh <models.json> <out-dir> [<wait-file>]`: the D2 liveness
  part. It waits for `<wait-file>` (default `~/m/CHAIN_DONE`), probes each model
  once, keeps those whose report says `check_kind: temporal`, and runs three
  repetitions. Build `<models.json>` locally with `records/d2probe.py` from the
  D1 parts (budget `max(60 s, 2.5 x D1 TLC time)`) for the `PROPERTY` models that
  `node/propmodels.py <selection.txt>` lists on a node.

Start each chain with `systemd-run` as in step 3. `ops/watch` prints new
progress lines from every node every five minutes.

### 5. Rebalancing

To finish sooner, later D4 repetitions moved to spot nodes:
`node/d4-trim.sh <rows>` stops a shard's D4 once `runs.jsonl` has `<rows>` lines
(the run in progress is discarded, never recorded), and `node/d4x.sh
<chunk-file>` runs the remaining repetitions on a spot node, one `dive-bench
--reps 1` call per line of `<tag> <workers-csv> <cfg>...`, for example
`s2r3-nano 1,4,8 specifications/NanoBlockchain/MCNanoLarge.cfg`. The chunk tags
appear in the D4 record's `benchmark_set_sha256`. `node/d2one.py` reruns chosen
D2 liveness repetitions on another node (each entry may name `reps`).

### 6. Pull and archive

On each node, pack everything the records need and upload it with a presigned
PUT, then note the SHA-256 for the record's `raw` field:

```sh
URL=$(scripts/perf/release/ops/presign.py put small-node-raw.tar.gz)
$N small "cd /home/bench && tar czf /tmp/small-node-raw.tar.gz r2 m hostinfo.txt chain.log && sha256sum /tmp/small-node-raw.tar.gz && curl -sS -T /tmp/small-node-raw.tar.gz '$URL'"
```

`ops/pull <node> <remote-file> <local-file>` fetches a single summary through
SSM in gzip+base64 chunks when the tarball is not needed. Keep pulled data
outside the repository. Terminate every node when its data is archived.

### 7. Build the records

From a dive checkout, with the pulled files in a scratch directory and the
golden directory `conformance/golden/tlc-2026.10.06.014338/examples`:

| Record | Commands |
|---|---|
| D1 | `dive-conformance r2 record --classes r2-classes.json --out <out.json> [--note TEXT]... r2-small.json r2-large-shard*.json`; add `attempts` and `raw` (archive digests) by hand |
| D2 | `records/benchagg.py runs.jsonl > agg.json`; `records/drecords.py d2live rows.jsonl <golden> > live.json` per batch; `records/d2spec.py agg.json <set-sha> <raw-sha> spec.json live*.json`; `records/mkrecord.py D2 spec.json <out.json>`. The `registered_divergence` fields and `liveness_all_agree_or_registered` were added by hand after checking `conformance/expected-divergences.toml`. |
| D3 | `records/drecords.py d3 rows.jsonl <golden> > d3.json`; `records/d3spec.py d3.json <raw-sha> spec.json`; `records/mkrecord.py D3 spec.json <out.json>` |
| D4 | `records/d4record.py d4.json raw.json setsha.json od-4:1,2,3:<runs.jsonl> spot-3:rep=2:<runs.jsonl> ...` (one `node:selection:file` per input); `records/d4spec.py d4.json spec.json`; `records/mkrecord.py D4 spec.json <out.json>`; `records/d4table.py <out.json>` prints the results-page table |
| D5 | `records/d5spec.py a/rows.jsonl b/rows.jsonl <golden> <raw-sha> spec.json`; `records/mkrecord.py D5 spec.json <out.json>` |
| D6 | `records/d6spec.py rows.jsonl <raw-sha> spec.json`; `records/mkrecord.py D6 spec.json <out.json>` |
| D7 | `node/d7-mac.sh <out-dir>` on the mac, from the repository root; then `records/d7record.py runs.jsonl startup.json sha.txt <raw-sha> <out.json>` |

`records/mktables.py <records-dir> <source8>` prints the D1, D2 and D3 tables
of the results page (its D4 branch predates the D4 record's shape; use
`d4table.py`). `records/benchruns.py` prints a compact view of a `dive-bench`
`runs.jsonl` and `records/closure.py` prints a module's closure, both for
inspection.

**0.1.0 constants to edit for the next campaign:** `mkrecord.py` (date,
source, binary digest, oracle, corpus, the host block), `d2spec.py`,
`d4spec.py` and `d7record.py` (revision in the command strings, `dive-bench`
digest, D1's SlushLarge time), `d5spec.py` (the bosco and trace figures quoted
in notes), `d4record.py` (the #1020 refusal text) and `mktables.py` (date).

## Sail (S1 to S7)

S1 to S6 compare Sail with the dive CLI; S7 compares Sail+dive with a fresh
TLC process per check. The Sail node is bootstrapped with
`sail/sail-node-ud.sh` (S1 to S6) or `sail/s7-node-ud.sh` (S7) as EC2 user
data, after prepending `AWS_REGION=`, `TOKEN_PARAM=` and `SAIL_REF=` lines;
everything else is driven through `ops/n` with the node's name in the state
file. Copy `sail/*` to `~/tools/` on the node.

- `sail/sail-build.sh` (S1 to S6) builds one Sail checkout, the staged worker
  and the `temporal_bench` example, and the dive CLI at the revision Sail
  pins. `sail/s7-build.sh <label> <sail-ref>` builds one Sail revision into
  `~/inst-<label>`; S7 used the labels `before`, `cache`, `mimalloc`, `final`
  and `lto` (commits in the S7 record's `sources`).
- `sail/temporal_bench.rs` is copied into Sail's `crates/sail-dive/examples/`
  on the node by the build scripts; it is not part of Sail or of the dive
  workspace.
- `sail/sail_temporal.py fixtures` builds the job fixtures from the Examples
  checkout and the goldens; `sail_temporal.py s1`..`s5` run S1 to S5, and
  `sail/sailrecord.py` builds their record.
- `sail/s7.py latency|first|loop|through|only <label>` runs one S7 part;
  `sail/s7-chain.sh` ran the `before`, `cache` and `mimalloc` parts, and
  `sail/s7-chain2.sh` built `final` and `lto` and ran the rest. `sail/s7-env.sh`
  points them at the pinned oracle's JRE and jar.
- `sail/s7record.py <raw-dir> <meta.json> <out.json> <tables.md>` builds the S7
  record and its tables; `sail/s7agg.py <raw-dir>` is a quick look while a
  chain runs.

`s7.py`, `sail_temporal.py` and `temporal_bench.rs` are byte-identical to the
files whose SHA-256 the S7 record lists under `harness_sha256`.

## Simulation throughput (S4)

`node/sim-chain.sh [--reps 3] [--workers 1,4,8] [--only BENCH]` runs
`node/simbench.py` as user `bench` on one bootstrapped node: each bench samples
the same number of traces of depth 100 at seed 0 in dive and TLC `-simulate`
(TLC gets `num=N/W`), alternating the tools, plus ten one-trace startups per
tool. It writes `~/sim/runs.jsonl` and `~/sim/hostinfo.txt`, then touches
`~/sim/DONE`. Drop the `argv` and `error` fields of each run before pulling;
they hold node paths. `records/simrecord.py --runs runs.jsonl --hostinfo
host.txt --source SHA --date DATE [--profile profile.json] --out OUT.json`
builds the record; the host lines and the profile summary are written by hand
from `hostinfo.txt` and `perf report`, without names, addresses or paths.

## Not kept

One-off probes and glue from the 0.1.0 campaign were left out: three scripts
that moved the tail of D2's second liveness batch from the small node to
shard 2 (`d2one.py` does the same job), Sail smoke and busy-refusal probes,
two Sail simulation probes, the script that assembled the results page from a
scratch template, and the generated spot-node user data (`bench-node.sh`
with `DRY_RUN=1` prints it).
