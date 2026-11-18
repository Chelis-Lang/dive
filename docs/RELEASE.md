# First release (v0.1): the first-value bar

[ADR 0153](decisions/0153-core-is-the-product.md) sets the direction. This file
defines when dive ships and what work is on the path to that release. It gives
no dates and no effort estimates. Progress is the measured gate numbers below.

## Who gets value first

The first user who gets value from dive is a TLA+ user with an existing
specification. It is often PlusCal and often has liveness properties with
fairness. They run `dive check` instead of TLC, with no extra flags. They get
the same answer, faster and without a JVM, or a clear refusal that names what is
not supported.

Everything on this page follows from that user.

## Gates

Each gate is measured on a commit on `main`. All six must hold to tag `v0.1.0`.

| Gate | Requirement | How it is measured |
|---|---|---|
| **R1 Coverage** | Default profile, **Core engine** (`--impl dive:core`), pinned Examples full tier, 1 worker. **0 disagreements.** Agreements plus registered expected divergences are at least 140. That is a frozen historical number: the count dive's first, since retired, checking engine reached on the same 164-model selection, where Core had 151, both with 0 disagreements. The historical 182/217 baseline (#486) spans four corpora, including 151 Examples agreements; re-measure before comparing. Every other model gives a named refusal or a declared budget stop. | `dive-conformance run --impl dive:core --impl-profile dive --corpus examples --tier all --workers 1`, compared with the frozen count of 140 |
| **R2 Speed** | End to end, including TLC's JVM startup, Core is faster than TLC on the geometric mean over the R1 agreeing models. A model slower than TLC gets a `perf` issue. A model more than 2x slower than TLC blocks the release. | One host, one worker, seed and fingerprint 0: pinned TLC (through the harness JRE) and `dive check` on every R1 agreeing model, wall clock including startup. Report the geometric mean of dive/TLC and list every model above 1x and above 2x. Golden TLC times from another host are triage only. The [R2 measurement procedure](developer/performance.md#r2-measurement-procedure) governs a gate run. |
| **R3 Bare use** | TLAPS and the CommunityModules ship inside dive at the lowest search priority, with provenance recorded. Ten original Examples specifications resolve with **no** `--module-path` or `--library-path` options. Nine must check successfully, including MCPaxos, MCBakery, at least two PlusCal specs and two liveness/fairness specs. EWD998ChanID may succeed or give its specific named VIEW/liveness or recursive-LET refusal (owner-approved bounded fallback); other failures block the gate. | A CI smoke test |
| **R4 Compat exists** | `--compat=tlc` runs on Core. Its known gaps are listed in `docs/tlc-differences.md`. It is not measured against an agreement target. | Small-tier run in the TLC profile reports no internal errors |
| **R5 Stable surface** | Per-command `--help` works. `--version` names the commit. Unimplemented commands such as `lsp` are hidden. The JSON report (`schema_version` 3) and the CLI help are pinned by golden tests. | Golden tests in the workspace gate |
| **R6 Shipped** | A release workflow builds Linux x86_64 and aarch64 archives with checksums. `cargo install --git … --locked dive-cli` is documented. The README is a user landing page. `CHANGELOG.md` has a curated `0.1.0` section. One release-candidate qualification (`cargo xtask gate --full`) passes on the tagged source. | Release workflow run on the tag |

The [release performance qualification](developer/release-performance-plan.md)
runs on the release-candidate source before the tag. Its R2 run is the gate
run; its other measurements are directional, and each regression gets a
`perf` issue.

Not required for v0.1:
- Core-native coverage beyond R1;
- distributed workers;
- DAP;
- checkpoint, spill or recovery breadth;
- IOUtils/IOEnv;
- TLC coverage and logical costs;
- LSP;
- compiled specifications;
- macOS as a gate;
- the deep tier.

## Critical path

In order. Each item has a GitHub issue in the
[`v0.1: first value`](https://github.com/Chelis-Lang/dive/milestone) milestone.

1. **Measure Core alone.**
   - Run the full tier on Core in both profiles.
   - Publish a refusal-reason histogram for Core.
   - Add a CI ratchet: the Core-native agreement count on the small tier may not
     fall.
   - Everything after this item is ranked by the histogram.
2. **Canonical sets in the default profile** (ADR 0153 §2).
   - Delete the normalization-history, receiver and authority paths that this
     makes unnecessary.
   - Register the D-entry.
   - MCQuicksort, ReadersWriters and MCAlternatingBit run natively.
3. **Burn down Core refusals by largest reason bucket,** not model by model.
4. **Core liveness at corpus scale.** Profile and fix MCInnerSerial first (#787).
   Then raise temporal capture capacity for the Examples liveness models that
   currently stop.
5. **Bundle the module libraries** and add the bare-use smoke test (R3).
6. **Make compat nominal on Core** (R4): the existing cheap switches plus a gap
   list.
7. **Make Core the only engine.** Done: the first checking engine and the
   `dive-check` crate are removed, keeping what `dive-trace` and Core start-up
   use, and golden comparison replaced the shadow gate (#791, #935).
8. **Freeze the surface** (R5), then **package and ship** (R6): one
   release-candidate qualification, then the tag.

## Status

The current numbers live in [scoreboard.md](scoreboard.md). The pinned tracking
issue "v0.1 first-value status" gets one comment per week with the R1–R3
numbers from the latest `main`.
