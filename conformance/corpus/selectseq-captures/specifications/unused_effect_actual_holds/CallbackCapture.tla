---- MODULE CallbackCapture ----
EXTENDS Naturals, Sequences, FiniteSets, TLC
VARIABLE x
Ignore(a) == TRUE
Svc == INSTANCE CallbackInner WITH q <- <<x,0>>, unused <- LET Test(y) == Ignore(Print(y,y)) IN SelectSeq(<<x,0>>,Test)
Init == x=0
Next == x'=1-x
P == Svc!P
====
---- MODULE CallbackInner ----
EXTENDS Naturals, Sequences, FiniteSets, TLC
VARIABLES q,unused
P == []<>(Len(SelectSeq(q,LAMBDA y: y=0))>=1)
====
