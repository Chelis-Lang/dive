# ADR 0078: Retain ordered worker errors through stopped-queue diagnostics

- Status: accepted bounded diagnostic
- Tracking: [issue 224](https://github.com/Chelis-Lang/dive/issues/224)

## Context

The exhausted-frontier transaction in ADR 0076 derives successful EOF from real
`run_levels` completion. A source worker can instead decide an invariant/action
property violation, incomplete successor, deadlock or evaluation failure. These
are stopped queues, not exhausted success. Pinned `Worker.run`,
`ModelChecker.setErrState`, `runTLC` and `AbstractChecker.modelCheck` retain their
first error even after later workers attempt other errors.

The complete 60-row `stopped-periodic-service/primary` selection preserves actual
integer `modelCheck` returns and independent CLI exits. The preceding complete
60-row development/rejected selections remain alongside it. At widths 1/4/8,
source division returns 2179/CLI 255, invariant 2110/12, action property 2112/13,
incomplete 2109/75 and deadlock 2114/11. A FALSE configured hook does not replace
the original source division or invariant. Failed Init returns 2103/75; successful
empty Init returns 0/0 with final checking. These are selected method/CLI
observations, not a proof of native clock, queue or graph equivalence.

## Decision

Add a distinct owned optional-hook diagnostic endpoint. It keeps the old
mandatory/optional/terminal APIs and the ordinary-check refusal marker intact.
Only an eligible ordered source stop with its real predecessor/source path may
enter an actual joined live pool's dispatch pause before timer/pool closure.

`StoppedQueue` is private-field borrowed authority. A private transaction fact is
minted immediately before its callback, carried through the engine outcome, and
checked against the original final winner and actual pool width before the
postjoin callback. A stateful opt-in flag, copied public verdict or zero queue
counter cannot manufacture that fact. Lane-close/dispatch/no-path failures have
no fact; callback error/unwind after entry can retain it. Callback unwind is
caught for this scoped diagnostic; process abort survival is not promised.

Diagnostic failures have separate typed flags and notes. They cannot replace the
source winner or mint successful EOF, final checking, a candidate or Holds.
Original source counters and postjoin/replay counters remain separate. Returning
`Complete` from the periodic control-flow kernel means its invocation returned,
not that exploration or temporal checking completed.

The failed finished-queue suspension branch skips configured hook, partial SCC,
checkpoint and resume. Hook-free unforced no-growth calls can instead return
without attempting suspension and update the original IEEE ratio. Force and
ratio remain the existing selected policy; no new defaults or ceilings.

## Consequences and limitations

Native graph evidence covers only the actual committed prefix. Java can call
`doNextCheckLiveness`/`addNextState` after an error winner; those writes are not
reproduced or credited here. Actual joins are retained, but no ordinary Java
queue/count/clock/model-proof equivalence follows. Failed Init, resource/library
faults, unsupported features and earlier candidate/hook stops do not enter the
worker-stop service. A shared-meter refusal preserves the original source error
without pretending that a service receipt was completed.

All deciding temporal candidates still use the shared private reconstruction and
fresh independent memory/JSON replay path. No caller word, graph, Valid flag,
EOF boolean or suspension flag is admitted as authority. Ordinary scheduling and
checkpoint composition remain explicit follow-up obligations.
