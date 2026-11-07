---- MODULE Small ----
\* Rewritten and unrewritten filters over SUBSET (SUBSET (1..3)) on both engines.
EXTENDS FiniteSets, Naturals
VARIABLES Edges, k
Init == /\ k \in 1..2
        /\ Edges \in {E \in SUBSET (SUBSET (1..3)) : \A e \in E : Cardinality(e) = k}
Next == /\ k' = 3 - k
        /\ \/ Edges' \in {E \in SUBSET (SUBSET (1..3)) : \A e \in E : Cardinality(e) <= k}
           \/ Edges' \in {E \in SUBSET (SUBSET (1..3)) : \A e \in E : e \in Edges}
           \/ Edges' \in {E \in SUBSET (SUBSET (1..3)) : \A e \in E : Cardinality(E) > Cardinality(e)}
           \/ Edges' \in {E \in SUBSET (SUBSET (1..3)) : \A <<a, b>> \in {<<1, 2>>} : a < b}
Inv == \A e \in Edges : e \subseteq 1..3
====
