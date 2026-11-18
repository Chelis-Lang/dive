# Progress review, 2026-11-05

An owner-requested critical review of dive after four weeks of agent-led
development (2026-10-08 to 2026-11-05): 472 commits and about 351k lines of Rust.
It records where the project stands, where the work went off course and what
changes. The owner's resulting decisions are in
[ADR 0153](../../decisions/0153-core-is-the-product.md). The release bar and
critical path are in [RELEASE.md](../../RELEASE.md).

Numbers come from git history, GitHub and the committed reports. An independent
verifier rechecked each one. Where it corrected a figure, the corrected figure
is used.

## Summary

dive is closer to useful than its status documents suggest, but further from a
release than its size implies.

- **The default engine gives honest answers.** No corpus disagreement is
  recorded in any tier.
- **The user-facing numbers have not moved since 2026-10-23 (small tier) and
  2026-10-26 (full tier).**
- **The work since then went into four things:**
  - a second engine that is not the default;
  - certification and evidence machinery;
  - per-phase qualification ceremonies;
  - features no user is waiting for.
- **There is no definition of a release** that anyone could finish.

## Where we are

| Measure | Value | Measured |
|---|---|---|
| Small tier, TLC profile, Reference engine | 150/156 agree, 6 unsupported, 0 disagree | 2026-10-23, not since changed |
| Small tier, default profile | 144 agree, 7 expected divergences, 5 unsupported | 2026-10-23 |
| SANY front end | 458/458 | |
| Full tier, TLC profile, 1 worker | 182/217 agree, 0 disagree (baseline #486) | 2026-10-26, never re-run |
| Full tier, default profile | never measured | |
| Full tier, Core engine alone | never measured | |
| Core small-tier shadow agreement | 250 → 250 → 250 → 250 → 252 → 246 of 312 over Phases 3–8 | 2026-11-04 |
| Core-strict Auto credit | 125 → 122 (TLC), 121 → 118 (default), Phase 7 → 8 | 2026-11-04 |
| Core default-profile speed | about 2.6x TLC geomean on a 10-model panel | 2026-10-28 |
| Reference speed | about 0.35x TLC at 1 worker | 2026-10-26 |
| Since 2026-10-26 | 187 commits; +115k lines of Rust, +77k lines of evidence JSON, +18k lines of Markdown; 0 new measured agreements | |
| Tracker | 79 open issues, 76 without a milestone, 72 unlabelled | 2026-11-05 |
| Local state | 170 worktrees, 215 branches | 2026-11-05 |

## Missteps and blind spots

Ranked by cost.

### 1. No finishable release target

[ROADMAP](../../ROADMAP.md), in the version this review replaces, defined release
as "full TLC parity": all 38 feature families, including:
- distributed workers;
- DAP debugging;
- checkpoint recovery;
- host I/O history;
- every Rust API.

"RC acceptance remains open" appeared in at least four places, and RC was never
defined. The crate version is `0.0.0` with `publish = false`, and there are no
release tags.

With no finite target, every phase "completed with findings" and the next one
began. No user got anything.

### 2. The user-facing metric was replaced by proxies, then stopped being measured

The full tier was last run on 2026-10-26. The default profile and the Core
engine have never been run on the full tier.

Phase reports tracked shadow, census and receipt counts instead, and those stayed
flat. The census agreed 1,215–1,222 times in every phase. Meanwhile
`docs/conformance.md` and #338 still quoted 181/217 from 10-24.

About 115k lines of Rust landed with no visible change to what a user gets, and
nobody could tell whether any of it closed a gap.

### 3. The second engine never became the product

ADR 0117 (2026-10-26) started the Core reboot because the Reference engine copied
TLC's semantics without its performance. The Core plan put things in this order:
1. safety core;
2. breadth;
3. modes;
4. default.

Phases 5–8 instead built the modes-class features on Core while safety-core
acceptance stayed open:
- spill;
- recovery;
- iterative deepening (DFID);
- simulation;
- authenticated distributed workers (#773);
- a DAP debugger (#774).

#773 and #774 merged on 11-04, after Phase 8 had already recorded a Core
regression.

ADR 0119 (2026-10-29) records the owner putting Core capability ahead of cutover.
That much of the delay was owner direction, and ADR 0153 now supersedes it. The
inverted sequencing inside Core was the agent's choice.

### 4. The "frozen" Reference engine kept growing

ADR 0117 says the old engine "gets bug fixes only". After it, 44 distinct
commits changed `dive-eval`, `dive-check`, `dive-value`, `dive-sim` and
`dive-model`, adding about 10k lines. That includes features such as #767 (final
Reference logical coverage) and #753. Seventeen commits edited both engines.

Every fix was paid for twice, and the engine meant for retirement got harder to
retire.

### 5. TLC's lazy-normalization history was emulated instead of escalated

The owner approved TLA+ comparisons as the default on 2026-10-08 (D-009). The
agent then kept "set construction still follows TLC's rules" in both profiles
(`docs/developer/core/02-semantic-contract.md` C2).

To do that, Core tracks per-object raw versus normalized history, receivers and
"authority" proofs. That produced the demand and receiver fix stream: #765, #775,
#778, #779, #781, #190, #612, #623 and #680, 51 commits in all.
- #765 (176 files, +8.5k lines) broke ReadersWriters, MCAlternatingBit and
  MCQuicksort.
- MCQuicksort, a plain set of singleton sets, is still refused.

The issue that tracks it (#781) forbids the simple fix outright. The owner
question that would have settled it (A6, `02-semantic-contract.md` §6 Q7) was
never brought to the owner.

TLC's history matters only when a set mixes values TLC cannot compare. That is a
property of ill-typed specifications. For every working spec the outcome does
not depend on history.

### 6. The process ceremony came back right after the owner's rapid-loop correction

Charter item 13 (2026-10-29) said to use focused checks and integrate small
changes, keeping full qualification for milestones and releases.

The agent then declared eight "phases" in seven days: #692, #706, #715, #720,
#732, #743, #764 and `81535879`, three of them on 11-01. Each ran:
- the 55-minute full gate;
- a 3,040-ID census;
- S3 archives, receipts, dollar ledgers and node-absence proofs;
- reports of bytes reclaimed by worktree cleanup.

The eight campaign commits alone add 19.4k lines of documentation. ADRs went
from 8 in the week before the correction to 33 in the week after.
Proof-vocabulary commit subjects ("certify", "prove", "authority", "replay")
rose from about 7% to 35%.

The correction changed the wording, not the behaviour.

### 7. Long-tail capabilities before any release

The following moved no agreement. Nothing stopped them, because no coverage
ratchet exists. Merged:
- distributed HMAC workers (#773, +5.6k);
- DAP (#774, +3.9k);
- Reference logical coverage (#767);
- candidate spill (#750);
- Core value reclamation (#766).

Open:
- host-I/O history work (#761, #758).

### 8. The bare user was never tried

The harness injects `--module-path` (`crates/dive-conformance/src/dive.rs`).
Without it, these fail:
- MCPaxos and MCBakery: "cannot find module TLAPS";
- EWD998ChanID: "cannot find module SequencesExt".

57 of 110 Examples directories extend TLAPS. In addition:
- `dive check --help` errors;
- `lsp` is advertised and unimplemented;
- the README reads as a status log.

The published parity overstates what a first-time user sees.

### 9. Performance work went to speculative caches, not known timeouts

Twelve rejected-experiment documentation commits (11.3k lines, 10-21 to 10-26),
rejected ADRs 0140 and 0147 and the Sailfish reuse probe found 2.98% prospective
repeats and shipped nothing.

MCInnerSerial, where TLC finishes in about 4 minutes, still stops in graph capture at the
900-second budget (#445). dive is at least 3.7 times slower there, and that has
never been profiled.

### 10. The tracker is a receipt log, not a plan

- 146 of 219 closed issues were closed the day they were opened.
- #498, the Core umbrella, has no comments, and every stage after the first is
  unchecked.
- #241 is still open with a 63k-character body.
- 15 issues carry "rescoped by #241" boilerplate with no acceptance test.
- Milestones M1, M5 and M6 are stale, and Phases 5–8 never got milestones.

GitHub shows no critical path, so the agent invented one every phase.

### 11. Status documents contradict each other and cannot be read quickly

Disagreements between documents:
- family counts: 8/30/0 in ROADMAP, 8/27/3 in `10-plan.md` and the scoreboard;
- Core speed: 2.6x versus 1.48x;
- full tier: 181 versus 182.

Of the scoreboard's 1,194 lines, about 1,000 were history, and of the
ROADMAP's 540 lines, 58%. Paragraphs bind commit and tree hashes, hedge every
claim ("not claimed", "remains open", "historical identity") and need forensics
to read. Status writing cost about two-thirds as much as code.

### 12. Workspace sprawl

The agent left 170 worktrees (about 20 GB) and 215 branches, 22 of them merged.
Evidence cites commits that are not on `main` (`2d55317a`, `00adce45`).

## Productivity diagnosis

There are three root causes:
- **No finite target.**
- **Risk aversion expressed as evidence maximalism.** Every claim got a
  certificate, an archive and a disclaimer.
- **No ratchet on the one number users care about.** Without it, effort drifted
  to whatever looked rigorous.

What did not happen at all: release engineering, packaging, bundled libraries,
or a single measurement of Core alone on the full tier.

The foundations are solid and reusable:
- **Front end.** The parser and semantic analysis agree with SANY on 458 of 458
  cases.
- **Values.** `dive-value` has good tests.
- **Conformance harness.**
- **Independent trace validator.**
- **Core engine.** Its speed is already well ahead of TLC where it runs.

The fix is direction, not a rewrite.

## What changes

Recorded in [ADR 0153](../../decisions/0153-core-is-the-product.md), the
[charter appendix](../../CHARTER.md) item 15, the standing direction in
[AGENTS.md](../../../AGENTS.md) and [RELEASE.md](../../RELEASE.md):

- **Core is the product.** The Reference checking runtime is retired once Core
  covers what it covers. `dive-eval` stays only as the independent validator's
  evaluator.
- **The default profile does the better thing,** starting with canonical sets.
  `--compat=tlc` exists on Core with documented gaps and gets no fidelity
  engineering.
- **The release is the first point of real user value,** measured by gates, with
  no dates.
- **Measure Core alone on the full tier,** ratchet that number in CI and burn
  down refusals by reason bucket.
- **Full qualification only for a release or cutover candidate.**
- **ADRs only for decisions,** with fewer worktrees and shorter writing.
