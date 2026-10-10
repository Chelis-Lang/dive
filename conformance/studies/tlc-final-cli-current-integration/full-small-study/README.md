# Fresh integrated full small selection

All 140 unchanged selected Examples models remain explicitly unsupported. There
are 138 raw subprocess records; two action-composition rows were refused by the
harness before launch. There are no agreements, disagreements, oracle errors or
timeouts. The actual harness exit is retained as 0: this does not certify model
checking or parity, because every selected model was refused.

The before/after manifests record the frozen evidence commit, actual release
binary hashes, transparent archive-launcher hash, source matrix and every raw
artifact. Source and executable bytes were verified unchanged. No old raw
observation or the first 140-row census was overwritten.

Reproduce on the recorded source with release binaries and a private verified
cache; use a new empty artifact directory, then invoke the portable `run.py`
with repository and release-directory arguments. The archived launcher only
relays byte-exact output from the separately hashed executable. The selection
is all small Examples models, one worker/job and a 600-second per-model bound.

This remains a correctness census, not a quiet performance benchmark or a
family certificate. The bounded safety-only route is a separate subsequent
implementation; this report gives it no credit.
