---- MODULE ConcreteSets ----
EXTENDS Naturals, FiniteSets
VARIABLES counter, bag
Init == /\ counter = 0 /\ bag = {[n |-> 1, msg |-> <<1, 2>>]}
Next == /\ counter < 1100 /\ counter' = counter + 1 /\ UNCHANGED bag
H == []<> (Cardinality(bag) = 1)
P == []<> (counter = 0)
C == []<> ((CHOOSE e \in bag : TRUE).n = 1)
====
