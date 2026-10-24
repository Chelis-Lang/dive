---- MODULE ConcatSecondTypeError ----
EXTENDS Naturals, Sequences
VARIABLE x
Init == x = 0
Next == x' = 1-x
P == []<>(SelectSeq(<<x>>, LAMBDA y: Len(<<y>> \o TRUE)=0) = <<>>)
====
