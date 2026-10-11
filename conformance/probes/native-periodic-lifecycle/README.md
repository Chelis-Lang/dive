# Native clock and dispatch lifecycle boundary

Issue #189 / ADR0052 connects real native dispatch authority, internally sampled
clock/controller polling, source-owned growth and actual safety EOF final SCCs.
It is diagnostic only: no ordinary schedule/stop-count or model proof credit.

`queue-primary/` preserves the separate exact producer:72 actual suspension/finish
controls plus24 empty-queue EOF controls, widths1/4/8, eight repeats each. These
execute real queue methods/threads, not model checking or arbitrary scheduler
proof. Native workers are job-pool workers; the dispatch mutex is operational
authority, not Java suspendAll equality.

Controller296 and numeric196+3 controls retain the rootcomponent producer in
`ordinary-periodic-work/` and `runtime-ratio/`. Native internal SystemTime
observations form a distinct source epoch and never retag those observations.

`development-final-snapshot/` preserves the actual first EOF panic, source/log.
Current EOF derives pointer sizes before recording last-check snapshots, even
when no timer ever ran. All candidates require fresh independent word/memory/JSON
replay before future verdicts. Current Run never reports Holds or violation.
