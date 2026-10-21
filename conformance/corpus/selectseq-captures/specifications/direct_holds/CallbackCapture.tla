---- MODULE CallbackCapture ----
EXTENDS Naturals, Sequences, FiniteSets, TLC
VARIABLE x
Init == x=0
Next == x'=1-x
P == []<>(Len(SelectSeq(<<x,0>>,LAMBDA y: y=0)) >=1)
====
