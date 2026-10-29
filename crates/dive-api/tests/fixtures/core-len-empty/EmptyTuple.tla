---- MODULE EmptyTuple ----
EXTENDS Integers, Sequences
VARIABLE f
Init == f = <<>>
Next == UNCHANGED f
Inv == Len(f) = 0
====
