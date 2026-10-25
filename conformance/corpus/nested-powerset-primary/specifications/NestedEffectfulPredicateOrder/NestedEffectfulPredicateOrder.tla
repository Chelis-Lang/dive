---- MODULE NestedEffectfulPredicateOrder ----
EXTENDS Naturals, FiniteSets, TLC
VARIABLES E
Init == /\ E \in {A \in SUBSET (SUBSET {1,2}) : \A e \in A : PrintT(<<"predicate",e>>) /\ Cardinality(e)=1}
        /\ PrintT(<<"accepted",E>>)
Next == UNCHANGED <<E>>
====
