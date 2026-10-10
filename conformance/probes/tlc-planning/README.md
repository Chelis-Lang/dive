# Bounded TLC planning projections

This is development evidence for issue #130 and ADR 0025. It is not a strict
integrated-liveness report and contains no edited or generated goldens.

`PlanningProbe.java` runs against the pinned TLC jar
`7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d`.
It uses inert state/action predicates for supplied-formula controls; evaluating
one fails the probe. It calls the jar's actual negation, simplification, DNF,
flattening and classifier methods, reflecting the private classifier structure.
The development adapter reproduces the grouping loop for visibility. Twelve
real models also call actual `FastTool` and `Liveness.processLiveness`, so their
actual bin and PEM arrays check that adapter independently.

All 84 supplied-formula controls are selected and retained: 79 complete
projections (including the empty input), four named action-shape refusals, and one Java-int DNF
cardinality failure. The failure preserves its preceding intermediate stages,
raw stderr and exit 1. All twelve real models are retained: eleven complete
projections and one SANY rejection of an invalid boxed action. There are no
model verdict, timing or strict-conformance claims in these numbers. Actual
promise/tableau projections are recorded for the next package; Rust does not
claim those identities yet.

A defect in the initial development adapter treated an explicitly supplied
empty conjunction as absence of fairness. The original adapter source,
80-case selection, metadata and raw files remain under `development-*`.
They are historical adapter-failure evidence, with no accepted-match credit.
The corrected selection retains those cases and adds deeper junctions and the
integer-overflow control. No case was dropped to obtain agreement. The complete
pre-whitespace-cleanup twelve-model sources, selection, metadata and raw capture
are also retained in `historical-pre-whitespace-models`, with hash verification
and no additional accepted-match credit.

Reproduce in a private verified cache and temporary compiled-probe directory:

```sh
python3 -I conformance/probes/tlc-planning/capture.py \
  --java "$DIVE_JAVA" --javac "$DIVE_JAVAC" --jar "$DIVE_ORACLE_JAR" \
  --reason 'explain this regeneration'
python3 -I conformance/probes/tlc-planning/capture-models.py \
  --java "$DIVE_JAVA" --javac "$DIVE_JAVAC" --jar "$DIVE_ORACLE_JAR" \
  --reason 'explain this regeneration'
cargo test -p dive-liveness --test tlc_planning --locked
python3 -I scripts/check-tlc-planning.py
```

Captures record the exact selection, command shape, reason, pin and SHA-256 of
every source/raw file. The verifier checks current and historical selections.
`source-matrix.json` identifies the native implementation/test tree used for
these controls. No shared cache source or golden is modified.

The native model entry point retains each property's configured identity and
predicate scope. Independent scoped-word controls exhaust all boolean words
through length four, all cycle starts and every word position for the eleven
accepted models: 5,390 stage/group comparisons. Wider supplied-formula words
use a fixed deterministic sample (499,200 stage/group truth comparisons),
checked directly before/after normalization
and after reconstructing EA/AE/group semantics. Negative Rust-caller controls
refuse impure/live/opaque captures and unchecked expression/definition IDs
without output in both profiles. Construction limits and TLC's DNF cardinality
ceiling fail explicitly before cartesian allocation. Formula-arena nodes and
references are charged cumulatively; group/PEM metadata has a derived bound from
those charged DNF branches and conjuncts, not aggregate byte accounting.

Strict integrated liveness, binary/tableau/promise identities, scheduler and
witness behavior remain unsupported. Neither the old 53-model final/short study
nor the ten-model lazy-domain report gains new credit from this planning work.

## Separate property-name fields

`TpLocal` configures `PROPERTY Live`, whose body forwards `[]<>(x = TRUE)`
through local `Alias(q) == Identity(q)`. Pinned `FastTool.getImpliedTemporals`
keeps the action name `Live`, but `getImpliedTemporalNames()` records the derived
role name `Identity`. Dive's role label correctly matches that derived vector.
The original configured-property table keeps `Live` separately. Four of the
controls demonstrate distinct action/configured and derived names; all eleven
completed controls match their respective original and derived name fields.
The probe records all three pinned tables rather than substituting a formula
helper's name for the configured property. Keeping action attribution through
future tableau/witness execution is a separate representation obligation
tracked in #134, not certified by normalization/grouping equality here.


The original native source matrix is immutable historical evidence. Its exact
source bytes are retained under `native-source-0b02e46/` and checked against the
unchanged `source-matrix.json`. The fallible evaluator/scope API migration is
tracked separately by `context-admission-source-matrix.json`; that record checks
the current implementation/test bytes and links to the historical matrix. The
same field/truth/negative controls still run; pinned selections and raw oracle
captures were not rewritten to accommodate this migration.

The evaluator admission integration preserves the prior source matrices exactly
against archived native source bytes. The separate
`conformance/probes/tlc-planning/combined-context-tableau-source-matrix.json`
hashes the current union of planning, context and particle/tableau controls.
Both historical and current controls are verified; no pinned selection, raw
transcript or earlier observation is replaced.

Public checker admission (#157) preserves the context-migration matrix and
its twelve exact source files under `native-source-context-admission/`.
`public-ir-admission-source-matrix.json` separately checks current source
against the same unmodified pinned oracle selection and raw captures.
