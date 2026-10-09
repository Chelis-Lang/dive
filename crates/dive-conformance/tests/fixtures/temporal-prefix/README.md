# Finite temporal-prefix exit classification

`QtEmpty` is the empty-domain existential temporal property captured by the
pinned oracle on 2026-10-09. These files were generated with the harness's
`dev capture` command; the command, exit status and raw output are retained.
They are copied unchanged from the temporal-quantifier probe corpus.

Reason: reproduce issue #77. At `94d0c50`, `LiveCheck.printErrorTrace` prints
temporal message 2116 and a one-state finite counterexample, but stores
`TLC_INVARIANT_VIOLATED_BEHAVIOR` (2110), giving exit 12. The observed counts
are 2 generated, 1 distinct, depth 1. Ordinary cyclic temporal violations
still exit 13. The harness checks this complete finite stream shape rather
than changing EC's general exit mapping.
