---- MODULE EmptySkipsCallbackError ----
EXTENDS Naturals, Sequences
VARIABLE x
Init == x = 0
Next == x' = 1-x
P == []<>(SelectSeq(<<>>, LAMBDA y: Len(<<y>> \o TRUE)=0) = <<>>)
====
