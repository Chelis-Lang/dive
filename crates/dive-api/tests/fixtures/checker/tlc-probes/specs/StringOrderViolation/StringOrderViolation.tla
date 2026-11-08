---- MODULE StringOrderViolation ----
\* Probe: set enumeration order decides which violation TLC reports. "zeta" is
\* interned before "alpha" because First appears earlier in the module, so in
\* TLC's order the set {"alpha", "zeta"} enumerates "zeta" first: y' = "zeta"
\* is generated first and InvZeta is reported after 2 generated states. A
\* checker that enumerates strings lexicographically generates "alpha" first
\* and reports InvAlpha instead. Both counterexamples have 2 states.
VARIABLE y
Init == y = "init"
First == "zeta"
Next == \E v \in {"alpha", First} : y' = v
InvAlpha == y # "alpha"
InvZeta == y # "zeta"
====
