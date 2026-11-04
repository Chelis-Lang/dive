---- MODULE LazyEffects ----
EXTENDS Naturals, TLC
VARIABLE x
Twice(a,b) == a /\ a
Init == /\ x=0 /\ Twice(PrintT("once"),Assert(FALSE,"unused"))
Next == x'=x
====
