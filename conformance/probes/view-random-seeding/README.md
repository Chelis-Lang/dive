# VIEW and random expansion: native admission boundary

This is a preserved native defect (#178), not a proposal to change TLA+ or TLC.
The old expansion path seeded from the raw predecessor, while pinned TLC's
`RandomEnumerableValues` asks `TLCStateMutExt.fingerPrint`, which applies the
configured VIEW (after any symmetry representative).

The [minimal module](before/RandomView.tla) has `V == n`, a random successor `x`,
and invariant `n < 3`. With seed 7, fingerprint polynomial 0 and one worker,
clean TLC and the old native checker both exited 12 with four generated/distinct
states and depth four. Their concrete `x` traces differed:

| Producer | Concrete x values |
| --- | --- |
| Clean pinned TLC | 0, 78, 49, 21 |
| Old native expansion | 0, 80, 94, 100 |

The [original index](before/index.json) pins the exact old native source commit,
executable SHA, oracle SHA, commands, source and raw output hashes. The original
bytes are unchanged. The compressed old engine and pinned
[TLC state fingerprint source](before/tlc-state-source/index.json) explain the
seeding distinction. Separate [validation controls](before/validation-index.json)
retain both concrete traces and their existing independent `Unsupported` result;
no false independent certificate was demonstrated.

Current ordinary safety checking conservatively refuses any configured VIEW
when the lowered source refers to a random native operator, or a configured
substitution selects one through its context alias/native override authority. Refusal precedes
binding, ASSUME, Init, output and search; actual workers are zero and statistics,
collision report and counterexample are absent. This includes unused random
definitions, aliases and typed native overrides. It is intentionally broader
than proven reachability: importing Randomization can include unused source
definitions that call random natives. The preserved initial positive-selection
failure in `development/pure-source-selection-before/` records that case; the
true pure positive imports only Naturals/TLC, with its assertions unchanged. Both profiles, both stores and requested 1/4/8 workers
with chaos are covered by `crates/dive-check/tests/view_random.rs`. Pure VIEW and
random checking without VIEW still run. SYMMETRY/random remains refused by its
existing independent limitation.

```sh
dive check conformance/probes/view-random-seeding/before/RandomView.tla --compat=tlc --seed 7 --fp 0 --workers 8 --output json
```

This now returns `unsupported` with the named transformed-fingerprint seeding
and independent-replay requirement. It does not claim the model is safe. Full
support requires matched concrete draws and independent replay, not just removal
of this guard or comparison of counts. Historical source authorities remain
archived separately from current-source integrity checks.

## Configured replacement control

A random replacement need not appear in any source call:

```tla
CONSTANT C(_)
Next == /\ n' = n + 1 /\ x' = C(1..100)
```

```text
CONSTANT C <- RandomElement
VIEW V
```

The initial node-only guard still ran that model. Its fresh source-natural
control and a public typed native replacement both produced the concrete `x`
values `0, 74, 75, 35`, four generated/distinct states, depth four and the
invariant violation. Those are failure evidence for admission, not a passing
oracle comparison. [The scratch source, outputs and interrupted gate](development/configured-native-before/index.json)
preserve the exact first guard producer. Current admission examines configured
RHS names through module contexts and native override names before binding.
The current engine controls include 336 no-work settings and 48 supported
positive settings; public file/API controls include 48 no-work settings.
The instantiated context control uses the public typed cfg name, retaining
rather than changing the textual cfg parser's token boundaries.

The ordinary no-VIEW RNG classifier has a separate audit obligation: hidden
configured native replacements can also bypass its node-only stream selection.
No algorithm change or seeding-equivalence claim is included in this repair;
source-natural clean-primary controls are tracked separately before that work.

## Frozen repair verification (2026-10-11)

[Current receipt](current/receipt.json) identifies the exact corrected producer
`4ecaa79d33a47e54126feaf171a854a2c77b641b`, full 961/961 gate,
96 checker CLI controls, 96 independent positive/invalid-Init witness controls,
24 compiled public Rust controls, and CM1 21/21 plus ValidationTestSuite 16/16 at
1/4/8 workers. The configured VIEW old wrong trace remains independently
`unsupported`; no false certificate is treated as valid. Concrete direct-RNG
compat positives reproduce the pinned `0,80,94,100` values. The earlier capture
script used the wrong JSON variant spelling; its failure/raw bytes remain in
`development/current-cli-protocol-before/`, and the final controls assert the
published `invariant-violation` field without changing implementation or oracle.

The separate [configured RNG defect #180](https://github.com/Chelis-Lang/dive/issues/180)
is now stronger than trace disagreement: [false-violation](no-view-configured-follow-up/false-violation/index.json)
has TLC success/native violation; [missed-violation](no-view-configured-follow-up/missed-violation/index.json)
has TLC violation/native success. Both immutable old `d110` and independently
reproduced current `b8fb13cdab443e7ad0206a467d9a392075f31397` outputs keep their
own producer identities. That soundness repair takes priority before broader
seeded recovery work. This VIEW-only delivery does not claim to fix it.
