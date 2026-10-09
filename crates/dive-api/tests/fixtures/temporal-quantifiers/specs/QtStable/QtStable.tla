---- MODULE QtStable ----
CONSTANT S
VARIABLE x
Init == x = 0
Next == x' \in S
Spec == Init /\ [][Next]_x /\ WF_x(Next)
P == \E i \in S: <>[](x=i)
====
