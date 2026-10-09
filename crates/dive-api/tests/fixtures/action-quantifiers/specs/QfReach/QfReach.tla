---- MODULE QfReach ----
EXTENDS Integers
VARIABLE x
Init == x=0
Move(i) == x=i /\ x'=i+1
Next == (\E i \in 0..1 : Move(i)) \/ (x=2 /\ x'=2)
Spec == Init /\ [][Next]_x /\ WF_x(Next)
P == <>(x=2)
====
