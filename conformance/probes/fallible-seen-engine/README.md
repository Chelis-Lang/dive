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
