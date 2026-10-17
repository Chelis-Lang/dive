---- MODULE RecValue ----
EXTENDS Naturals, Sequences
VARIABLE x
RECURSIVE Sum(_)
Sum(s) == IF s = <<>> THEN 0 ELSE Head(s)+Sum(Tail(s))
Init == x=0
A == \E i \in 1..Sum(<<x,1>>) : x<2 /\ x'=x+1
Next == A \/ UNCHANGED x
Spec == Init /\ [][Next]_x /\ WF_x(A)
Hold == <>(x=2)
Fail == []<>(x=0)
====
