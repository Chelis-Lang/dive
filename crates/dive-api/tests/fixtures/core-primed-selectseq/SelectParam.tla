---- MODULE SelectParam ----
EXTENDS Integers, Sequences
CONSTANT Test(_)
VARIABLE s, cut
ByValue == SelectSeq(s, LAMBDA n : n = cut)
ByOp == SelectSeq(s, Test)
====
