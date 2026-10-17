---- MODULE RecValue ----
EXTENDS Naturals, Sequences
VARIABLE x
Offset(n) == LET RECURSIVE Go(_)
                 Go(k) == IF k=0 THEN x ELSE 1+Go(k-1)
             IN Go(n)
Init == x=0
A == \E i \in 0..Offset(0) : x<2 /\ x'=Offset(1)
Next == A \/ UNCHANGED x
Spec == Init /\ [][Next]_x /\ WF_x(A)
Hold == <>(x=2)
Fail == []<>(x=0)
====
