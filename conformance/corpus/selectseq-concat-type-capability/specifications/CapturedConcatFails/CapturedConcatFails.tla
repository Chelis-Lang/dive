---- MODULE CapturedConcatFails ----
EXTENDS Naturals, Sequences
VARIABLE x
Init == x = 0
Next == x' = 1-x
Svc == INSTANCE CallbackInner WITH q <- <<x,0>>, unused <- LET F[i \in 0..1] == IF i=0 THEN <<>> ELSE <<x>> \o F[i-1] IN F[1]
P == Svc!P
====
---- MODULE CallbackInner ----
EXTENDS Naturals, Sequences
VARIABLES q, unused
P == []<>(SelectSeq(q,LAMBDA y: y=0) = (<<1>>))
====
