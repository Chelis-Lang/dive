---- MODULE CallbackCapture ----
EXTENDS Naturals, Sequences
VARIABLE x
Forward(F(_),s) == SelectSeq(s,F)
Captured(k) == LET Test(y) == y=k IN Forward(Test,<<k,0>>)
Init == x=0
Next == x'=1-x
P == []<>(Len(Captured(x)) >=1)
====
