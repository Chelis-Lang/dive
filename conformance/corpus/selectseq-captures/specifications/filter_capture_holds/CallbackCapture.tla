---- MODULE CallbackCapture ----
EXTENDS Naturals, Sequences, FiniteSets, TLC
VARIABLE x
Svc == INSTANCE CallbackInner WITH q <- <<x,0>>, unused <- {v \in {0,1}: v=x}
Init == x=0
Next == x'=1-x
P == Svc!P
====
---- MODULE CallbackInner ----
EXTENDS Naturals, Sequences, FiniteSets, TLC
VARIABLES q,unused
P == []<>(Len(SelectSeq(q,LAMBDA y: y=0)) >=1)
====
