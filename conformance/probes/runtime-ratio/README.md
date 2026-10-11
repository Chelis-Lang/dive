# Pinned runtime-ratio arithmetic

`RatioProbe.java` calls the unchanged `ModelChecker.updateRuntimeRatio` from the
pinned 1.8.x jar (revision `94d0c50`, SHA-256
`7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d`).
It controls the previous ratio and start timestamp, with before/after samples
around the real wall-clock read. All 196 selected updates completed; every
observed bracket was zero milliseconds. Three negative elapsed-time controls
triggered the original assertion with Java assertions enabled. The original
missing-directory constructor failure is retained and earns no runtime credit.

Independent readback reproduces Java signed-long wrapping, long-to-double
conversion, `Math.max` NaN/positive-zero behavior and division, comparing raw
canonical double bits within every observed clock bracket. The Rust fixture
retains all selected rows. NaN bits use Java's canonical output convention;
finite values, infinities and signed zero are compared exactly.

This proves the separately supplied arithmetic component. It does not certify
clock origin, monotonicity, progress scheduling, checkpoint persistence, graph
quiescence or an ordinary model-checking verdict. Negative elapsed time is an
explicit native refusal; the reference assertion control is not a claim that
TLC enables Java assertions in every user invocation. The signed integer
interval is the actual configured Java field, not an inferred duration.

See `docs/ordinary-periodic-controller.md` and ADR0050. Runtime commands and raw
producer paths remain in the capture; source and binaries are historical inputs,
not the current integrated implementation's producer identity.
