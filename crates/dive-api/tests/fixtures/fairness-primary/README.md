# Post-Next temporal primary controls

These original minimal inputs are retained from the independently pinned
`tlc-2026.10.06.014338` audit (source `94d0c5070852b37b801bb8998c436383ab7ee37b`).
`oracle.json` records the existing generated primary golden hashes and observations;
no golden or raw oracle output is edited or copied into this fixture directory.

`MinChooseFair` fails both Next and the fair action; `MinChooseFairSafeNext`
fails only the fair action. Their pinned primary is GENERAL 1000/exit 255, with
1 distinct state and respectively 1/2 generated states. The implementation does
not yet reproduce the actual reached fairness-row and diagnostic replay pipeline.
The first case therefore refuses its unproved post-Next primary; the latter
retains its existing temporal-analysis refusal.

`MinChooseOtherFair` contains harmless unrelated fairness, while `MinChoosePlain`
has no fairness conjunct. Each still configures a genuine temporal property.
The existing audit observed physical exit75 for both but did not produce classified
goldens. They must never be converted to GENERAL from fairness syntax. Unproved
non-EvalException Next endings with pending temporal demand are explicit refusals.
`NoTemporal.cfg` removes all temporal obligations and preserves the existing
safety-only Next error behavior. Successful fairness and measured 2179 GenError
controls remain in the existing core-liveness suite.

Pinned Worker.run catches a Next failure and still calls doNextCheckLiveness.
Its reached row-error replay can clear keepCallStack; ModelChecker.doNextFailed
then preserves the first error code. Ordinary Next replay alone does not prove
that primary or a temporal error word. An unconfirmed mandatory word earns no
agreement credit. Issue #502 remains open for the reached fairness/error pipeline.
