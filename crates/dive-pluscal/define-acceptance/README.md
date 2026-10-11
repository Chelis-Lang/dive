# Bounded global DEFINE evidence

The [user guide](../../../docs/pluscal-definitions.md) describes implemented helper declarations, their limits, complete Cargo examples and Full-checksum behavior. This directory retains the pinned translator selection and observations separately from native delivery and later integration sources.

Run the always-on evidence check from the repository root:

```sh
python3 -I crates/dive-pluscal/define-acceptance/verify-delivery.py
```

It verifies the original scoped source archive, closed gate logs, all 4,524 portable runtime records, and actual results from 288 model settings plus 12 valid and 48 invalid independent replay controls. The records preserve original file content; they do not reproduce the producer's filesystem or symlink layout. Original executable hashes stay in the delivery record, with executable verification available separately through `--artifacts`.

## Source qualifications

The original runtime was uncommitted, with its parent and 185 directly relevant files identified in `delivery.json` and `runtime-source/manifest.json`. It is never retagged as a later delivery or integration. The two listed gate-only repairs describe runtime-to-final-gate test changes, rather than every test changed from the parent.

The previously existing algorithm, macro and public rewrite controls were promoted when bounded DEFINE became supported. The original scoped runtime archive omitted the macro test source; its original current gate map omitted that source too. This is a retained historical source-qualification gap. `root-readback/readback.json` states it explicitly. `root-readback/test-source.json` and its archives record five exact committed promotion and gate-repair test inputs as post-delivery evidence. They cannot retroactively establish historical runtime source authority. The combined source receives a fresh gate before publication.

The archived publication-consumer controls reject corrupted raw records and an invented original source-map attribution, and pass after exact originals are restored. No primary observations, oracle goldens, quotas, assertions or feature-family statuses were changed to obtain those results.

## Accounting

The original raw selection retains all 136 cases: 80 exact outputs, 18 invalid primary artifacts without admission credit, 36 named refusals and two reference rejections. Public API controls retain all 272 outcomes. AST/CRC equality and generated artifact admission do not prove model correctness. Finite model and independent witness controls are separate evidence; the whole PlusCal semantic family remains uncertified.
