---- MODULE InstInner ----
\* Instantiated by Inst.
EXTENDS Naturals
CONSTANT C, F(_)
VARIABLE v
Init == v = C
Step == v < C + 3 /\ v' = F(v)
Inv == v >= C
Twice == v + v
Lazy(z) == IF v > 100 THEN z ELSE v
Grows == v' > v
====
