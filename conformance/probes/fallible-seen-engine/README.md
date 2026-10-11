# Fallible disk seen-state safety increment (#212)

This study extends the independently archived library-only #205 producer with an
explicit ordinary `INIT`/`NEXT` safety route. It does not certify F-031, parallel
disk reads, graph/temporal retention or recovery. See [ADR0068](../../../docs/decisions/0068-fallible-disk-seen-safety.md)
and [the executable CLI/Rust guide](../../../docs/user/disk-seen-safety.md).

The original eight-model memory selection covers duplicate Init and successors,
invariant/constraint/deadlock/evaluation stops, pure VIEW and safety symmetry.
It is captured before implementation at both profiles/stores, inline and requested
1/4/8 workers, and no-chaos/0/7 schedules. The clean primary is independently
captured at seed7/fp0/one worker. Later disk comparisons preserve all selected
concrete observables; requested/actual worker and explicitly added disk evidence
are compared separately. Tiny resource and file-integrity/cleanup controls are
native typed failures, not invented Java error-class matches.

Required original and mutant witnesses use the standalone validator, not the
checker or storage membership map. An incomplete storage-failure trace remains
diagnostic evidence, never full agreement. Physical key records and successfully
admitted keys are distinct from predecessor/frontier publication.

Development failures are retained with their original producer/source and raw
logs. The first merged baseline had incomplete conflict resolution and failed to
compile; corrected source was compiled and captured before engine implementation.
Focused compile/lint corrections are distinct from any eventual immutable full
gate. Runtime receipts and source archives will record exact frozen producers;
this introduction itself makes no claim that the full delivery gate has closed.

Only explicit `INIT`/`NEXT` configurations are admitted. All configured PROPERTY
and SPECIFICATION roles, graph/source capture, temporal modes, DFID and native
checkpoint/recovery refuse the combination before endpoint work. Caller-provided
bound models can already contain earlier startup output; capture/DFID perform no
new assumption or search work. Callback-free role decomposition and immutable
concurrent disk read views remain future increments.


The first full engine gate (`b07aaa1164d04ccb6a15f29df5b6046ab599443b`)
closed all eight stages with 1,264/1,265 tests passing and zero skipped; its new
CLI orphan-policy assertion mismatched the actual plural diagnostic, and two
current source maps retained stale documentation hashes. Full failed logs/source
remain separate from the next corrected epoch. The CLI test now checks the exact
measured plural message; no usage exit, no-output or pre-file assertion changes.

[Invariant replay before repair](invariant-replay-before/) records a distinct
observed gap: clean primary/resident3 distinct/1 queued versus disk4/2.
Current disk mode refuses the actual invariant-error replay boundary by name,
preserves its original error and diagnostic word, and audits cleanup. The
ordinary memory path and its call-stack replay remain unchanged; no supported
matrix case is removed to conceal this refusal.
