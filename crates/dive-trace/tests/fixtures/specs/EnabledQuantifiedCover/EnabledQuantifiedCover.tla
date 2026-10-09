---- MODULE EnabledQuantifiedCover ----
EXTENDS Integers
VARIABLES x,y
Init == x \in -1..4 /\ y \in -1..4
Next == UNCHANGED <<x,y>>
Move(i) == x'=i /\ y' \in 0..2 /\ x'+y'=x+y
Any == \E i \in 0..2 : Move(i)
All == \A i \in {0,1} : x'=i
EmptyAny == \E i \in {} : x'>y'
Inv == /\ ((ENABLED Any) = (x+y \in 0..4))
       /\ ~(ENABLED All)
       /\ ~(ENABLED EmptyAny)
====
