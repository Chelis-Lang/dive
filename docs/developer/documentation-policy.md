# Documentation is part of feature delivery

Every user-visible feature or behavior change updates its user guide in the same
PR. This is an acceptance obligation during implementation, not a separate final
phase that blocks unrelated engineering work.

The user guide states what runs now, how to invoke it through each exposed CLI/API,
what the report and exits mean, defaults and TLC counterparts, source/module
requirements, supported/refused combinations, resource and proof qualifications,
and at least one checked runnable example. Link full evidence without making the
reader understand internal work-package names to use the feature.

Developer documentation records design, constraints, provenance, independently
verified behavior, source/binary identity, known defects and remaining acceptance
obligations. Maintain the relevant ADR, issue, scoreboard and change log. Every
new TLC limitation also updates both the registry and its stable D-entry in the
user-facing differences log. Keep failed experiments and historical producer
files intact; label them historical instead of letting their old scope read as
current functionality.

When a feature changes status, update the manual index/support boundary, its
feature page, README/current status and affected API rustdoc together. Do not
announce a parsed flag, completed planning artifact or supported subset as full
parity. Planned default changes stay explicitly planned until code and controls
prove them. Mark competing descriptions obsolete by linking the current contract,
while preserving the historical measurement section and raw evidence.

Validate example commands against the recorded binary and source checkout.
Check module/config names, resolved options, schemas, exits and relevant
positive/negative results. Compile exposed Rust examples through rustdoc or an
existing example/test where practical. Check repository-relative links. Do not
weaken conformance to make an example appear supported; document the refusal or
choose an actually supported example and retain the failing observation.
