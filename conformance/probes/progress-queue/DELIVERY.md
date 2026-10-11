# Closed producer-owned request diagnostic

Production is `ce0007152798eb46481845557a85d49e40dee4eb`. The full local gate
passed 42 stages and 1,017 tests, zero skipped, on unchanged committed Rust/Cargo
inputs. The Cargo guide and all 37 required safety models passed at 1/4/8.

[The user guide](../../../docs/producer-progress-requests.md) explains exact
claimed/returned/unwritten/committed facts, quotas and ordinary-checker refusal.
[The gate receipt](gate-current/after.json),
[closed runtime](gate-current/runtime-closed.json),
[closed-consumer readback](gate-current/closed-consumer-readback.json), and
[delivery hash index](delivery-index.json) retain separate source and binary facts.

The 33 raw reference rows observe actual local Worker/queue/runTLC methods with
controlled instrumentation distinguished from natural execution. They certify
neither native ordinary queue suspension nor winning statistics. Native request
receipts are captured separately in gate and runtime epochs (40 each), not added
as model coverage. Isolated old-timer/new-request replay directories preserve
the otherwise identically named model outputs without conflating the two routes.
New-request 4 and actual EOF 8 freshly replay Valid in memory/JSON; 48 semantic/shape
mutants are Invalid. Original timer 4 controls remain separate.

The quota 31 row preserves real safety EOF followed by a final-SCC resource stop;
temporal completion remains false. Known expansion cuts cancel requests while
earlier claims drain to the original ordered winner. Prior failed driver, test
and style observations are retained, with no success credit.

No ordinary CLI schedule, model proof, count certificate or whole-family credit
is granted. Next work is actual completed-prefix safety/output/Phase-L publication
under request quiescence before controller/hook/SCC checks.
