---- MODULE PcScopes ----
EXTENDS Naturals
VARIABLE x
Init == x=0
Next == x'=1-x
Hit(i) == x=i
Visit(i) == []<>Hit(i)
Stable(i) == <>[]Hit(i)
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Pair(a,b) == []<>(Hit(a) /\ ~Hit(b))
P == Pair(0,1) /\ Pair(1,0)
====
