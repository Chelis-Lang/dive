---- MODULE CallbackCapture ----
EXTENDS Naturals, Sequences, FiniteSets, TLC
VARIABLE x
Init == x=0
Next == x'=1-x
P == []<>(Len(SelectSeq(<<x,0>>,LAMBDA y: \E z \in {y}: z=0)) =0)
====
