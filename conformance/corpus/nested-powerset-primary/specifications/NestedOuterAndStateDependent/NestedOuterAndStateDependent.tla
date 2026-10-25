---- MODULE NestedOuterAndStateDependent ----
EXTENDS Naturals, FiniteSets, TLC
VARIABLES v,E
Init == /\ v \in {0,1}
        /\ E \in {A \in SUBSET (SUBSET {1,2}) : \A e \in A : Cardinality(e)=v /\ Cardinality(A)<=1}
        /\ PrintT(<<"accepted",v,E>>)
Next == UNCHANGED <<v,E>>
====
