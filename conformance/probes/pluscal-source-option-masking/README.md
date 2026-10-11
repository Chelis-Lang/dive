# Source declarations inside an existing translation block

The pinned translator sees `\* PlusCal options (-wf)` even when it appears
inside a previous translation block. Its fresh output retains `WF_vars`.
The frozen option producer instead masked that block, accepted Full and
body-only replacements, and lost fairness. `verification.json` records the
original executable and source hashes; both original stdout documents and
primary source/output are retained. These are failed producer observations,
never current agreement or model proof.

Native high-level rewriting now checks the original bounded source before
masking an old block. A recognized option declaration yields `Unsupported`;
no artifact, replacement, CRC or proof is returned. This also applies when
`replace_modified` is granted and under both compatibility profiles. Supplied-
body low-level editing keeps its separate nonproof contract. Source option
parsing/precedence remains the follow-up in issue #182 and D-021.
