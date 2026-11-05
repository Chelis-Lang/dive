# 0153: Core is the product; the default does the better thing; release at first value

- **Status:** accepted (owner decision, 2026-11-05)
- **Date:** 2026-11-05

## Context

On 2026-10-26, ADR 0117 started a new execution core. The reason was that the
original ("Reference") engine copied TLC's semantics without TLC's performance.
Ten days later:
- Reference is still the default.
- Both engines are being changed.
- The Core shadow agreement is flat or falling (small tier 250 → 246 of 312).
- The full tier has not been measured since 10-26, and Core alone has never been
  run on it.
- No release is defined. The ROADMAP's release scope was full TLC parity across
  38 feature families.

A large share of recent Core work tracks TLC's per-object lazy-normalization
history: raw versus normalized children, receivers and "authority". It does this
in both profiles (`docs/developer/core/02-semantic-contract.md` A6 and C1–C2).
That history changes results only for sets that mix values TLC cannot compare,
such as an integer with a string or a record with a tuple. Those are ill-typed
specifications.
- When every pair of elements is comparable, sorting gives one result whatever
  the insertion order or prior normalization.
- The owner already chose TLA+ comparisons as the default (D-009, 2026-10-08).

The review in
[2026-11-05-progress-review.md](../reviews/2026-11-05-progress-review.md) has the
evidence.

## Decision

1. **Core is the product engine.** The goal is TLC functional parity with better
   performance, followed by removing the scaffolding that got us there.
   - The Reference *checking runtime* gets no new features. That covers the
     breadth-first search, liveness graph, simulation and DFID drivers, and the
     Auto fallback routing. It is removed once Core alone matches its full-tier
     coverage (release gate R1 in [RELEASE.md](../RELEASE.md)).
   - Until then, only fixes for wrong user-visible results land in it.
   - The shared front end, binder and `dive-eval` stay where Core startup or the
     independent validator `dive-trace` still needs them.
   - The shadow comparison against Reference is replaced by the goldens when
     Reference is removed.
2. **The default profile does the better thing** wherever TLC's behaviour is an
   artifact of its implementation. The first application is sets:
   - Sets are canonical in the default profile. Core keeps no TLC normalization
     history, receivers or construction "authority".
   - This extends D-009 from comparison to set construction. It answers
     02-semantic-contract §6 Q7 (A6).
   - The implementing PR registers the divergence (D-entry) and deletes the
     history and authority paths it makes unnecessary.
3. **`--compat=tlc` is a nominal feature.** It must exist on Core, select the
   cheap TLC-observable switches that already exist, and document its known gaps
   in `docs/tlc-differences.md`.
   - Exact TLC fidelity in compat mode is not a goal, and no significant
     engineering is justified only by it.
   - This relaxes the CHARTER appendix item 10 requirement that compat "must pass
     full conformance against the pinned TLC".
   - Where compat cannot reproduce TLC cheaply, it refuses by name or keeps
     canonical behaviour with the gap documented.
4. **Release at first real value, and measure it by gates.** The first user to
   get value runs `dive check` on an existing spec, often PlusCal and often with
   liveness, instead of TLC. They get the same answer, faster and with no JVM and
   no extra flags, or a clear named refusal.
   - The gates are in [RELEASE.md](../RELEASE.md).
   - Aim for 100% coverage. Do not keep a fallback tier if it costs real
     development effort; a named refusal is acceptable for what Core does not
     cover yet.
   - Plans, milestones and status give no dates or effort estimates.
5. **Frozen until the first release:**
   - distributed workers;
   - DAP and debugging;
   - checkpoint, spill and recovery breadth;
   - IOUtils/IOEnv history;
   - TLC coverage and logical costs;
   - Reference features;
   - speculative performance caches;
   - LSP.

   Merged code in these areas stays, and is undocumented as a user feature where
   it is incomplete.
6. **Working method.** The owner's 2026-10-29 rapid-loop correction is restated
   with limits, in [AGENTS.md](../../AGENTS.md) "Standing direction":
   - full qualification (full gate plus census) only for a release or
     engine-cutover candidate;
   - ADRs only for decisions;
   - a weekly re-measurement of the release metric.

This supersedes ADR 0119 item 5 (cutover deferred) and every statement that
Reference remains the default as a goal. It also supersedes the "profile: both"
TLC set-construction rule in `02-semantic-contract.md` C2 for the default profile.

## Consequences

- **The per-model receiver and demand fix stream ends.** #765, #775, #778, #779,
  #781, #190, #612, #623 and #680 collapse into one canonical-sets change.
  Specifications that mix incomparable values in one set may get an answer in
  the default profile where TLC raises an error. That difference is registered.
- **Core coverage, measured alone, is now the critical path.** Its refusal-reason
  histogram is the backlog.
- **Compat-mode issues are low priority** unless they also affect the default
  profile.
- **Removing the fallback is part of the release, not a later project.** A
  release that still routes through Reference must justify the cost of keeping
  it.
- **Revisit:** if a real user specification depends on TLC's set-construction
  error, add a targeted compat behaviour then, not before.
