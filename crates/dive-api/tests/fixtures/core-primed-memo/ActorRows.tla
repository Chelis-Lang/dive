---- MODULE ActorRows ----
EXTENDS Naturals
CONSTANT Actors
VARIABLES w, stamp
Init == /\ w = [a \in Actors |-> [bit |-> 0, seen |-> 0]]
        /\ stamp = 0
Update(a) == /\ w' = [w EXCEPT ![a].bit = 1 - @, ![a].seen = stamp]
             /\ stamp' \in 0..2
Next == \E a \in Actors : Update(a)
TypeOK == /\ stamp \in 0..2
          /\ \A a \in Actors : /\ w[a].bit \in 0..1
                               /\ w[a].seen \in 0..2
====
