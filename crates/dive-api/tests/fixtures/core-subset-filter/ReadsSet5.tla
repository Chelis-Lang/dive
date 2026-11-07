---- MODULE ReadsSet5 ----
\* The body reads the filtered subset itself, so no rewrite applies.
EXTENDS FiniteSets, Naturals
VARIABLE Edges
Init == Edges \in {E \in SUBSET (SUBSET (1..5)) : \A e \in E : Cardinality(e) = 2 /\ E # {}}
Next == UNCHANGED Edges
====
