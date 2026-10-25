---- MODULE NestedNonBooleanAfterEmpty ----
EXTENDS Naturals, FiniteSets, TLC
VARIABLES E
Init == /\ E \in {A \in SUBSET (SUBSET {1,2}) : \A e \in A : 1}
        /\ PrintT(<<"accepted",E>>)
Next == UNCHANGED <<E>>
====
