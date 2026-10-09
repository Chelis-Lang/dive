---- MODULE PcStable ----
EXTENDS Naturals
VARIABLE x
Init == x=0
Next == x'=1-x
Hit(i) == x=i
Visit(i) == []<>Hit(i)
Stable(i) == <>[]Hit(i)
Spec == Init /\ [][Next]_x /\ WF_x(Next)
P == \E i \in {0,1}: Stable(i)
====
