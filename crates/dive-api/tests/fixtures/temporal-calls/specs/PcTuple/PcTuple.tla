---- MODULE PcTuple ----
EXTENDS Naturals
VARIABLE x
Init == x=0
Next == x'=1-x
Hit(i) == x=i
Visit(i) == []<>Hit(i)
Stable(i) == <>[]Hit(i)
Spec == Init /\ [][Next]_x
VisitPair(pair) == []<>(x=pair[1] \/ x=pair[2])
Inc(i) == i+1
Wrap(i) == VisitPair(<<i,Inc(i)>>)
P == Wrap(0)
====
