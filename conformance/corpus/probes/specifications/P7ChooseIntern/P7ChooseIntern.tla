---- MODULE P7ChooseIntern ----
\* Probe: identical to P2Choose's string case except that an unused
\* definition mentioning "alpha" appears earlier in the module. If CHOOSE
\* follows string interning order, the chosen element changes even though
\* the semantics of Init did not.
EXTENDS Integers
Unused == "alpha"
VARIABLES s
Init == s = CHOOSE e \in {"zeta", "alpha", "mid"} : TRUE
Next == UNCHANGED s
SLex == s = "alpha"
====
