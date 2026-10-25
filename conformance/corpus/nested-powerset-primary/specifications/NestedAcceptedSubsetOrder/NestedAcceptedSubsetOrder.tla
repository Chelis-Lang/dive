---- MODULE NestedAcceptedSubsetOrder ----
EXTENDS Naturals, FiniteSets, TLC
VARIABLES E
Init == /\ E \in {A \in SUBSET (SUBSET {1,2,3}) : \A e \in A : Cardinality(e)=2}
        /\ PrintT(<<"accepted",E>>)
Next == UNCHANGED <<E>>
====
