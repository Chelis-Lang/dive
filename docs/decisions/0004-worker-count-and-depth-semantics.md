# 0004: Worker count, search depth and counterexample length

- **Status:** accepted
- **Date:** 2026-10-08

## Context

Soundness contract rules 1 and 2 ask `dive` to match TLC's distinct-state count, generated-state count and search depth, and to produce safety counterexamples of the same length as TLC's ("both are shortest by breadth-first search"). Rule 7 requires worker-count invariance.

TLC does not provide that invariance:
- With more than one worker, TLC's search is only *approximately* breadth-first. See Lamport's comment in `tlc2/tool/TLCTrace.java` (lines ~99–110).
- The reported depth varies with worker count. This is acknowledged in the TLC usage text (`TLC.java`) and in tlaplus/tlaplus#883.
- The Examples CI deliberately skips comparing `stateDepth` for this reason.
- Distinct and generated counts for *completed* exhaustive runs do not depend on worker count. The Examples CI relies on this.

## Decision

1. Goldens that record depth, counterexample length or a counterexample trace come from TLC run with **`-workers 1`**. That is the only configuration where TLC's BFS is exact.
2. Distinct and generated state counts may be compared against TLC runs at any worker count. The harness's worker-invariance mode (for example `--workers 1,4`) measures and reports TLC's own invariance, so any TLC variance is recorded as an oracle fact and not blamed on `dive`.
3. `dive` is deterministic at every worker count: same counts, same depth, same chosen counterexample. Its counterexample is a shortest one, compared with the single-worker TLC golden by length and validity (rule 2), not byte for byte.

## Consequences

Generating goldens is slower, because it is single-threaded. That cost is acceptable since goldens are generated rarely. Multi-worker TLC runs remain useful as invariance probes.

## Amendments

**2026-10-08 (harness review).** How the harness applies this ADR:

1. **The relaxation is for TLC only.** Leniency for multi-worker runs applies only when the implementation under test is itself a TLC oracle (`ComparePolicy::oracle_leniency`). It covers:
   - counts, compared only after a complete search;
   - depth, queue size, counterexample length and end-kind, and violated property, which are not compared.

   Any other implementation, `dive` included, is compared with the single-worker golden on every field at every worker count. Any variance across its worker counts is a `disagree` (decision 3).
2. **TLC's outcome is its single-worker comparison.** For a TLC oracle, a model's outcome is the single-worker run against the golden. Multi-worker TLC runs are reported only in the report's worker-invariance section, as oracle facts (decision 2), and never as a model `disagree`. The variance is real:
   - with four workers TLC gives different counts and depth after a safety violation (Examples `DieHard`);
   - it names a different set of violated temporal properties (`MCDistributedReplicatedLog`);
   - it can even change the verdict when the spec reads the worker count (`MCDieHardest` asserts `TLCGet("config").worker = 1`, so at four workers it is an assumption failure).
3. **Worker-introspecting specs.** A model whose module mentions `TLCGet("config")` is marked `worker_introspecting` in the report, because its behaviour may legitimately depend on `-workers`. The detection is a plain text search with whitespace ignored. `dive` is still held to the single-worker golden for these models; the flag explains a variance, it does not excuse one.
4. **Excluded points.** Runs that time out, hit a resource limit or are unclassified are left out of the variance judgements, and are counted as `excluded`.
5. **ADR 0010 (profiles and options).** The oracle's `-workers 1` is a golden-generation constraint, not a limit on `dive`. `dive` runs at any worker count and must equal the single-worker golden at each one.

   Implementation options (`--impl-option`, e.g. `seed`) are recorded in every report. An option the implementation does not declare result-preserving (e.g. `int=big`) makes every would-be `agree` a `divergence-mode` outcome. Such runs are never counted as agreeing with TLC goldens. Comparing them against a separate expectation set is future work.

   Every report also records the implementation's profile, set with `--impl-profile` or `DIVE_COMPAT=tlc`:
   - `dive`, the default;
   - `tlc`, pure TLC behaviour, including TLC's defaults for the worker count, seeding and exit codes;
   - `tlc+overrides=<features>`.

   It also records `strict_tier`, which is true only for pure `tlc` without result-changing options. Under `dive`, a disagreement listed in `conformance/expected-divergences.toml` (glob, profile or option, expected verdict, justification, link into `docs/divergences.md`) is `expected-divergence`, never `agree`. Under `tlc` and `tlc+overrides` the registry never applies, and an entry claiming those profiles is a configuration error.

## Amendment: correctness, not just depth (2026-10-08)

Single-worker goldens are also required for *correctness*. The pinned TLC has a multi-worker race in value normalisation (tlaplus#1445), which can produce bogus states and liveness counterexamples. Multi-worker TLC runs remain useful only as invariance probes, and their results are never goldens. See `docs/survey/oracle-history.md`.

## Amendment: worker-introspecting models under `--compat=tlc` (2026-10-09, issue #55)

Lead decision (from WP-D3, PR #54). Examples `MCDieHardest` asserts `TLCGet("config").worker = 1`. Under `--compat=tlc` at N > 1 workers dive reports the physical worker count, as TLC does (D-011), so the `ASSUME` fails exactly as it does in TLC at `-workers N`. Comparing that run with the single-worker golden made it a `disagree`, although dive does what TLC does. Where this amendment differs from the sections above, it holds.

1. **Worker-specific goldens.** For a model the harness flags `worker_introspecting` (amendment 3 above), `golden generate --workers N` (N > 1) records TLC's own result at `-workers N` in a separate file, `<model>.workers-N.json`, beside the single-worker golden, with `workers: N`. It is written only for flagged models and only with a recorded `--reason`. It never replaces or touches the single-worker golden, which stays the golden of decision 1. This is the one exception to "multi-worker TLC runs are never goldens" (amendment of 2026-10-08): these goldens are compared on the verdict and error class only. For `MCDieHardest` TLC decides both before the search starts (an `ASSUME` on the configuration), so the normalisation race of tlaplus#1445 cannot reach them; a flagged model whose multi-worker verdict depends on the search would need its worker-specific golden checked for that race before it is recorded.
2. **Comparison.** A run of a flagged model under the `tlc` profile (or `tlc+overrides`) at N > 1 workers is compared with the worker-specific golden at N, if there is one, on the verdict and the error class only: TLC's multi-worker counts, depth and counterexample are approximate (decisions 1 and 2). The run is left out of the worker-invariance judgement, since the variance there is TLC's own. A counterexample in such a run is still validated (ADR 0016). The golden must match the pinned corpus, oracle and model entry and record a reason; otherwise the run is an `oracle-error`. Without a worker-specific golden, the single-worker golden applies on every field, as before, and the report says so.
3. **The default profile is unchanged.** It reports the logical worker count, 1, at every `--workers` setting (D-011), so the single-worker golden applies at every worker count, worker-specific goldens or not.
4. **Goldens generated.** `MCDieHardest` at 3, 4 and 8 workers: `assumption-failure`, `TLC_ASSUMPTION_FALSE` at each. With them, `run --impl dive --filter 'specifications/DieHard/MCDieHardest.cfg' --workers 1,3,4,8` agrees in both profiles: before, the `tlc` profile disagreed at 3, 4 and 8 workers.

## Numeric-exit qualification (2026-10-13)

The historical worker-introspecting amendment's "verdict and error class only"
phrasing omitted a required observable. [ADR 0096](0096-truthful-default-numeric-exit-conformance.md)
corrects both ordinary and worker-specific comparison to include the exact
physical numeric exit, including an absent code. Worker-specific counts, depth
and witness shape remain outside this comparison; independent counterexample
validation remains required. Numeric exits are never waived by worker/count
leniency. For the pinned `MCDieHardest` assumption at 3/4/8 workers, deciding
EC2104 has actual exit 10, not safety exit 12. No golden or classifier changes.

The earlier observations and original floors remain historical text, qualified
by ADR 0096's accounting correction. Fresh complete measurements and review
remain required; this qualification approves no replacement floor or fresh
parity certificate.
