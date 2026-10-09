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
8. ~~**Canonical `CHOOSE` order.**~~ **Answered 2026-10-08: (a)**, canonical order by default, with TLC's order under `--choose-order=tlc` / `--compat=tlc` (ADR 0010). Original question: TLC's `CHOOSE` over strings follows the interning order of string tokens in the module text, so adding an unused definition can change a `CHOOSE` result (probes P2 and P7 in `docs/survey/competitive.md`). This breaks the charter's metamorphic rule. Should `--choose-order=canonical` become the default, with TLC's order under `--compat=tlc`? Or should TLC's order stay the default? The metamorphic tests would then need to exempt string `CHOOSE`.
9. ~~**Exploration order in the default profile**~~ **Answered 2026-10-08: (a)**, canonical exploration by default, with TLC's order under `--compat=tlc` (D-010). Original question (ADR 0015 §7):
   - **What canonical order changes.** Under the approved canonical value order, sets of strings and model values are also *enumerated* canonically: in `\E`, `x' \in S`, filters and maps.
     - Complete searches are unaffected: the same states, totals and depth.
     - At a violation, the default profile can stop at a different event than TLC. It may report a different violated property among violations at the same depth, different counts at the stop, or a different shortest counterexample of the same length.
     - Probe `StringOrderViolation` in `crates/dive-check/tests/fixtures/tlc-probes/`: TLC reports `InvZeta`, and canonical order would report `InvAlpha`.
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
14. ~~**Default liveness counterexample.**~~ **Answered 2026-10-09:** deterministic `--lasso=short`, with a shortest prefix and a documented short cycle. Build and prove TLC's selection under `--lasso=tlc` / `--compat=tlc`. D-013.
15. ~~**Symmetry with liveness.**~~ **Answered 2026-10-09:** `--symmetry-liveness=unreduced` replaces refusal as the intended default. Disable symmetry reduction for the entire liveness run and report it. Keep `refuse` and TLC's quotient behavior as explicit modes; `--compat=tlc` selects the latter. Until M6 implements these modes, the checker still refuses symmetry and temporal properties. D-005.
16. ~~**Non-bijective symmetry functions.**~~ **Answered 2026-10-09:** reject them by default with a diagnostic naming the function; preserve TLC's permissive behavior under `--compat=tlc`. D-015.
17. ~~**Symmetry audit.**~~ **Answered 2026-10-09:** implement `--check-symmetry` as an opt-in audit of the model's declared symmetry. D-016.
18. ~~**Periodic liveness scheduling.**~~ **Answered 2026-10-09:** use deterministic level-boundary checks by default; preserve TLC's wall-clock schedule under `--compat=tlc`. Goldens continue to use final checking. A level-boundary run is not labelled precisely TLC-equivalent merely because TLC could stop at that boundary. D-014.
