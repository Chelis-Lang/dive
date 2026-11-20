# Open questions for the owner

Each question is open until it is answered here or recorded in an ADR.

1. ~~**License.**~~ **Answered 2026-10-08: MIT** (ADR 0007). Original question: Which licence should dive use: MIT (the older org repos), `Apache-2.0 OR MIT` (barnacle), or something else? It must be decided before any public release (ADR 0007).
2. ~~**Integer semantics opt-in.**~~ **Answered 2026-10-08:** big integers are the intended default, and TLC 32-bit behaviour is built and proven under `--int=tlc` / `--compat=tlc` (ADR 0010). Original question: Should arbitrary-precision integers be an early named divergence, or deferred until after M5? TLC raises an overflow error at 32 bits; see `docs/divergences.md`.
3. ~~**Liveness plus symmetry.**~~ **Answered 2026-10-08:** dive refuses by default, and TLC's warn-and-proceed is built and proven under `--symmetry-liveness=tlc` / `--compat=tlc` (ADR 0010). Original question: Should `dive` refuse liveness checking under symmetry by default, as a divergence from TLC, which only warns? The charter defaults to matching TLC, so this needs an explicit owner call.
4. ~~**Oracle cadence.**~~ **Answered 2026-10-08: a single snapshot for now, with no cadence** (ADR 0002). Original question: How often should the re-hosted TLC 1.8.x snapshot be refreshed? Proposed: quarterly, or when the Examples corpus pin moves, with a recorded golden diff.
5. ~~**Parallel workers by default.**~~ **Answered 2026-10-08: yes.** Result-preserving enhancements may default to on (ADR 0010).
6. ~~**Role of v1.7.4.**~~ **Resolved 2026-10-08:** the 1.8.x line is the oracle, and v1.7.4 is reference-only (ADR 0002 revision). Original question:
   - **The problem.** v1.7.4 branched from upstream master on 2020-04-25, and no CommunityModules release tried runs with it. It therefore runs without CommunityModules, so every Examples spec that uses them disagrees on that oracle (ADR 0003).
   - **The question.** Should v1.7.4 stay a cross-check for core semantics only? Or should a second, older 1.8.x snapshot replace it as the drift oracle?

   Either way the decision is recorded.
7. ~~**Module library pins.**~~ **Resolved 2026-10-08:** they move together with the oracle as one bundle (ADR 0002 revision). Original question: The TLAPS library (tlapm `library/`) and Apalache's modules are now pinned by commit, as upstream Examples CI uses them unpinned. Should they follow the same refresh policy as the oracle, which is currently a single snapshot (ADR 0002)?
8. ~~**Canonical `CHOOSE` order.**~~ **Answered 2026-10-08: (a)**, canonical order by default, with TLC's order under `--choose-order=tlc` / `--compat=tlc` (ADR 0010). Original question: TLC's `CHOOSE` over strings follows the interning order of string tokens in the module text, so adding an unused definition can change a `CHOOSE` result (probes P2 and P7 in `docs/internal/survey/competitive.md`). This breaks the charter's metamorphic rule. Should `--choose-order=canonical` become the default, with TLC's order under `--compat=tlc`? Or should TLC's order stay the default? The metamorphic tests would then need to exempt string `CHOOSE`.
9. ~~**Exploration order in the default profile**~~ **Answered 2026-10-08: (a)**, canonical exploration by default, with TLC's order under `--compat=tlc` (D-010). Original question (ADR 0015 §7):
   - **What canonical order changes.** Under the approved canonical value order, sets of strings and model values are also *enumerated* canonically: in `\E`, `x' \in S`, filters and maps.
     - Complete searches are unaffected: the same states, totals and depth.
     - At a violation, the default profile can stop at a different event than TLC. It may report a different violated property among violations at the same depth, different counts at the stop, or a different shortest counterexample of the same length.
     - Probe `StringOrderViolation` in `crates/dive-api/tests/fixtures/checker/tlc-probes/`: TLC reports `InvZeta`, and canonical order would report `InvAlpha`.
   - **The options:**
     - **(a)** Exploration follows the canonical order, keeping the default profile text-independent, which is D-006's purpose. dive flags runs where the two orders disagreed on an enumerated set, and the harness treats at-violation differences on flagged runs as expected divergences under D-006.
     - **(b)** Keep TLC's order for exploration in both profiles, and use canonical order only for `CHOOSE`. Then adding an unused definition can again change which violation is reported.
   - **Recommended:** (a).
   - `--compat=tlc` is unaffected either way.
   - **Built both ways.** `dive-eval` implements both orders (`ExplorationOrder`, D-010). Until this is answered, the default profile explores in TLC's order, option (b); answering (a) changes one line of `Options::dive`.
10. ~~**`TLCGet("config").worker`**~~ **Answered 2026-10-08: logical** (1 at any `--workers`), with the physical count under `--compat=tlc` (D-011). Original question (ADR 0014 §7):
    - **The proposal.** Report the *logical* worker count, 1, at every `--workers` setting. dive's parallel search is by construction TLC's single-worker search (ADR 0015), and the harness holds dive to the single-worker golden. `MCDieHardest` asserts `worker = 1`. The physical count is in dive's own output.
    - **The alternative.** Report the physical count, as TLC does for its own workers. A spec could then fail at `--workers 8` exactly as TLC fails at `-workers 8`.
    - **Recommended:** logical.
11. ~~**Layering change for M5**~~ **Resolved by lead 2026-10-08: accepted.** `dive-model` is in `xtask/src/layering.rs`, `dive-eval` has the `enumerate` feature, `check-layering` rejects a `dive-trace` build that enables it, and the workspace gate builds `dive-trace` standalone. ADRs 0013 and 0016 are accepted. Original question (ADRs 0013 §8 and 0016 §2): Approve:
    - a new crate, `dive-model`, which ports TLC's `SpecProcessor` and binds the configuration. It would depend on `dive-ir`, `dive-value`, `dive-config`, `dive-eval` and `dive-stdlib`, and `dive-check`, `dive-liveness`, `dive-sim`, `dive-trace`, `dive-compile` and `dive-api` would depend on it;
    - `dive-eval`'s successor enumerator behind a cargo feature, `enumerate`, which `dive-trace` keeps off. It would be enforced by a standalone `dive-trace` build in the workspace gate and a `check-layering` rule.

    The trace-independence rule is kept: `dive-trace` still does not depend on `dive-check`, `dive-liveness`, `dive-sim` or `dive-store`.
12. ~~**Comparisons TLC refuses but TLA+ answers.**~~ **Answered 2026-10-08: TLA+ comparisons by default**, with TLC's rules under `--compat=tlc` (D-009). Original question: TLC raises an error for some `=` and `\in` tests whose answer TLA+ defines, and its errors are asymmetric and order-dependent (ADR 0011; fixtures `Asym*`, `ErrMemRcdTuple` and `ErrMemProductRec` in `crates/dive-value/tests/fixtures/`):
   - `[a |-> 1, b |-> 1] = <<1>>` is an error, but `<<1>> = [a |-> 1, b |-> 1]` is `FALSE`. Both are functions with different domains, so TLA+ says `FALSE`.
   - `<<1>> \in [a : {1}]` and `[a |-> 1] \in {1} \X {2}` are errors. TLA+ says `FALSE`.
   - `{[a |-> 1, b |-> 1], <<1>>}` is a set, but `{<<1>>, [a |-> 1, b |-> 1]}` is an error.
   - `1 \in {1, "a"}` is `TRUE` while `"a" \in {1, 2}` is an error, because TLC normalizes set literals lazily.

   **Proposal:** an opt-in `--comparisons=tla` (TLC mode `=tlc`, part of `--compat=tlc`). It answers `=` and `\in` between functions, records, tuples and sets as TLA+ defines them, symmetrically and independently of element order. It keeps the error only where TLA+ leaves the answer unspecified, such as `1 = "a"` or `{1} = 1`, where an error is the sound response. This is result-changing: specs that TLC rejects with an evaluation error would get a verdict. Should it become the default, stay opt-in, or not be built? Today dive matches TLC exactly in both profiles. See D-009 in `docs/tlc-differences.md`.

   The `=` and `\in` rules are built in `dive-eval` (`Comparisons::Tla`, tested in `crates/dive-eval/tests/options.rs`) but off by default; making them the default changes one line of `Options::dive`.
13. ~~**Host I/O from specifications.**~~ **Answered 2026-10-09:** require explicit `--allow-io=env,files,exec` grants in every profile, including `--compat=tlc`. Report restricted compatibility and external inputs; implement environment and file access first, defer process execution. Ungranted operators remain named refusals. D-017.
14. ~~**Default liveness counterexample.**~~ **Answered 2026-10-09:** deterministic `--lasso=short`, with a shortest prefix and a documented short cycle. Build and prove TLC's selection under `--lasso=tlc` / `--compat=tlc`. D-013. **Amended 2026-10-12 (rescue, ADR 0086):** TLC's exact lasso is no longer a parity obligation; `--lasso=tlc` is withdrawn and `--compat=tlc` keeps the short validated lasso.
15. ~~**Symmetry with liveness.**~~ **Answered 2026-10-09:** `--symmetry-liveness=unreduced` replaces refusal as the intended default. Disable symmetry reduction for the entire liveness run and report it. Keep `refuse` and TLC's quotient behavior as explicit modes; `--compat=tlc` selects the latter. The unreduced/refuse modes now implement this policy for supported temporal runs; TLC quotient checking remains a named refusal. D-005.
16. ~~**Non-bijective symmetry functions.**~~ **Answered 2026-10-09:** reject them by default with a diagnostic naming the function; preserve TLC's permissive behavior under `--compat=tlc`. D-015.
17. ~~**Symmetry audit.**~~ **Answered 2026-10-09:** implement `--check-symmetry` as an opt-in audit of the model's declared symmetry. D-016.
18. ~~**Periodic liveness scheduling.**~~ **Answered 2026-10-09:** use deterministic level-boundary checks by default; preserve TLC's wall-clock schedule under `--compat=tlc`. Goldens continue to use final checking. A level-boundary run is not labelled precisely TLC-equivalent merely because TLC could stop at that boundary. D-014.

## Owner direction of 2026-11-05 (ADR 0153)

These are answered:
- **Default engine and cutover.** Core is the product. The Reference checking
  runtime is removed once Core alone meets release gate R1. This supersedes ADR
  0119 item 5.
- **History-dependent `elements()` and set construction** (A6;
  `developer/core/02-semantic-contract.md` §6 Q7 and C2). Sets are canonical in
  the default profile, with no TLC normalization history. `--compat=tlc` is
  nominal, and its gaps are documented.
- **Release scope.** The first release is the first-value bar in
  [RELEASE.md](RELEASE.md). There are no dates and no effort estimates.

**Rule for open questions.** TLC bookkeeping variants already follow the
owner's correct/faster default ruling (ADR 0153): register their precise
observable differences without repeat escalation. Questions about actual
mathematical semantics or soundness remain open until resolved; unproved cases
retain a named refusal.

- **Bundled libraries (R3).** Ship TLAPS (BSD-2-Clause) and the CommunityModules (MIT)
  inside dive at the lowest module-search priority. *Recommended and default:
  yes.*
- **R3 checking requirement (#789), answered.** The owner requested a bounded solution attempt, then a named refusal if it could not be solved. All ten original Examples must resolve; nine must check successfully. Only EWD998ChanID may retain its specific named refusal. Report checked/refused counts separately; no Core coverage credit follows. #428 remains deferred.
- **Default `--time-limit`.** *Recommended and default:* none in either profile,
  matching TLC. A user-set limit prints a prominent stop notice. Implemented:
  neither profile has a default time limit.
- **v0.1 platforms.** *Recommended and default:* Linux x86_64 and aarch64
  required; macOS arm64 best effort. Distribution is GitHub releases plus
  `cargo install --git`. crates.io comes later.
- **`ENABLED` cutoff (02-semantic-contract §6 Q8).** *Recommended and default:*
  drop the TLC artifact in the default profile if it is one, as with B2 and B4.
- **Remaining 02-semantic-contract §6 questions (Q1–Q6).** *Default:* the
  canonical or better behaviour in the default profile, and whatever is cheap in
  compat.

- **Recursive Boolean action predicates and generated counts (#813), resolved by the owner, 2026-11-06.** Keep the faster default and register the precise predicate-witness count divergence (D-029), with its nominal compat gap. The minimal control generates TLC 3 versus Core 2 states, both 1 distinct at depth 1; original VoteProof generates 318,877 versus 27,727, both 6,962 distinct at depth 14. Those two count controls retain every other observable and independent word-validation obligation. Pure Boolean short-circuiting also avoids unnecessary alternatives; the pinned OrError result is registered separately, without a general error waiver. Plans with compiled effects or unknown calls refuse, and recursive calls/arguments are bounded with budget polling. Lexical-capture, prime, escape, operator and other soundness guards remain. The standing correct/faster ruling covers bookkeeping variants without repeated owner questions, with precise registrations rather than a blanket count waiver.

- **Simulation with liveness, superseded.** The 2026-11-06 answer (a named refusal) is replaced by the owner's request for at least TLC's simulation capability, liveness included; see [ADR 0162](decisions/0162-simulation-liveness-and-scale.md) and [the simulation design](developer/simulation-design.md). The VIEW ruling below concerns exhaustive checking; simulation closes loops only on concrete states, so it needs no VIEW-soundness argument.
- **VIEW with liveness, answered by the owner, 2026-11-06.** Stays a named refusal past v0.1. The `VIEW` is what makes these state spaces finite, so a sound quotient liveness check first needs a VIEW-soundness argument (#428). Affects `FiniteMonotonic/MCDistributedReplicatedLog` and `ewd998/EWD998ChanID`; the latter is consistent with the R3 ruling above.
- **SmokeEWD998_SC, out of scope.** It is a host script that launches a nested TLC through `IOUtils!IOEnvExec`, not a model; it is not counted as a Core refusal.

## Simulation defaults and semantics (ADR 0162), answered by the owner, 2026-11-20

The owner accepted [the simulation design](developer/simulation-design.md#15-owner-rulings-2026-11-20):

1. **Extending truncated traces (D-034):** `--liveness-extend auto` is the
   default in the default profile; `0` under `--compat=tlc`.
2. **`RandomElement` in sampled traces:** draw at random and validate each
   draw as some element of its set, coordinated with #910.
3. **Unbounded sets (D-035):** sampled by default in the default profile with
   a stated, seeded distribution that the report names, together with the
   sets sampled; `--compat=tlc` keeps TLC's GENERAL 1000 error.
4. **Report retention:** the deciding trace plus a bounded, validated sample by
   default; `--keep-traces all` and `--trace-dir` give complete output, and
   the user guide explains how to get it for export, animation and test
   generation.

## Native recovery counter default (issue152)

Pinned TLC recovery resets generated and initial counts to recovered distinct
states. The native snapshot can retain the original totals as explicit
`--checkpoint-counters retain`; the default remains `tlc` until approved.
Recommendation: make retained totals the default for the default profile once
resume controls prove uninterrupted counts/trace equivalence, and keep TLC reset
under `--compat=tlc`.

## Finite liveness runtime ratio (D-014, under consideration)

The pinned zero-duration updater can produce NaN and the controlled controller
skips later unforced checks for that value. Consider a separately named
`--liveness-ratio=finite` option that defines a finite zero-duration policy. It
is not implemented, no new default is approved, and the TLC-equivalent setting
must preserve the recorded IEEE arithmetic. Component evidence and the native
diagnostic ResourceLimit experiment are qualified in [D-014](internal/tlc-differences-notes.md#zero-duration-ratio-edge-case-controlled-component-evidence).

## Checker CLI exit default (resolved 2026-10-14)

**Resolved by the lead under the owner's delegated judgment toward eventual
parity:** ordinary `dive check` selects TLC physical statuses without changing
evaluator semantics. `--exit-codes native` preserves the historical CLI route;
Rust `CheckReport::exit_code()` retains its historical profile contract. This
is a new explicit decision, not an implication of ADR 0096 or the expert's
earlier plan. See [ADR 0097](decisions/0097-check-exit-conventions.md). Actual
new corpus measurements and all existing floors remain separate obligations.

## Run-budget default values (ADR 0107, D-026)

Tests, the gate and the harness pass explicit budgets.

- **Default `--time-limit` (default since the 2026-11-05 batch above):** none
  in either profile, matching TLC. A user-set limit that runs out reports a
  `stopped` result with the resource, limit and a continuation hint, exit 5.
- **Default `--max-memory` (answered by the owner, 2026-10-20):** half of
  physical memory in both profiles, rather than the JVM's quarter. TLC's
  default footprint is its heap plus off-heap fingerprint storage and its disk
  queue, and dive must keep at least TLC's capacity.
- **ADR 0107 status (answered by the owner, 2026-10-20):** accepted.
- **Gate budgets (answered by the owner, 2026-10-20):** the gate's
  `--timeout 1800` moves to the TLC-derived budgets in a follow-up PR opened
  right after this policy merges. Each model that exceeds its budget is filed
  as a performance issue.

## R2 fixed-set geomean

The R2 procedure (`docs/developer/performance.md`) reports two geomeans: the
gate's mean over agreeing models, and a fixed-set mean over every model with a
TLC time that counts a stop at max(2, its lower bound) and a refused or
excluded model at 2. Dropping a slow model lowers only the first.

- **Recommended:** report both; only the agreeing-model mean gates, as
  `docs/RELEASE.md` says now. Open for the owner: should the fixed-set mean
  also have to be below 1x?

## Deductive proof system scope

Should dive support proof handoff to existing TLAPS, maintain a companion
around `tlapm`, or share its frontend with a separately maintained Rust
reimplementation of TLAPS? First measure cold and edited proofs against the
current upstream manager and prove the handoff on real artifacts. A new
manager needs a repeatable improvement over upstream or a thin companion,
independent evidence for accepted obligations, and an accountable maintainer.
Approve a full implementation and its trust contract separately.
[Options and acceptance tests](internal/survey/verification-workloads.md#tlaps-options-and-evidence)
