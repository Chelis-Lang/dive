---- MODULE RecValue ----
EXTENDS Naturals, Sequences
VARIABLE x
RECURSIVE Even(_), Odd(_)
Even(n) == IF n=0 THEN TRUE ELSE Odd(n-1)
Odd(n) == IF n=0 THEN FALSE ELSE Even(n-1)
Init == x=0
A == \E i \in 1..(IF Even(x) THEN 1 ELSE 2) : x<2 /\ x'=x+1
Next == A \/ UNCHANGED x
Spec == Init /\ [][Next]_x /\ WF_x(A)
Hold == []<>(Even(x))
Fail == []<>(Odd(x))
====
