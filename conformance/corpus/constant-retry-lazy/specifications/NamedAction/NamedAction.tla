---- MODULE NamedAction ----
EXTENDS Integers
VARIABLE x
F == [i \in 0..2 |-> IF i=0 THEN 0 ELSE 1 \div 0]
Init == x=0
Next == x'=1-x
A == x'=x+1 /\ F[0]=0
Spec == Init /\ [][Next]_x /\ WF_x(A)
P == <> (x=2)
====
