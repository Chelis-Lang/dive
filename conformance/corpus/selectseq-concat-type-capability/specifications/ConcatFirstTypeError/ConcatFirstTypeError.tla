---- MODULE ConcatFirstTypeError ----
EXTENDS Naturals, Sequences
VARIABLE x
Init == x = 0
Next == x' = 1-x
P == []<>(SelectSeq(<<x>>, LAMBDA y: Len(TRUE \o <<y>>)=0) = <<>>)
====
