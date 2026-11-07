---- MODULE DivZero ----
\* The body fails on singletons: the filter raises the error as without a rewrite.
EXTENDS FiniteSets, Naturals
VARIABLE Edges
Init == Edges \in {E \in SUBSET (SUBSET (1..3)) : \A e \in E : 2 \div (Cardinality(e) - 1) > 0}
Next == UNCHANGED Edges
====
