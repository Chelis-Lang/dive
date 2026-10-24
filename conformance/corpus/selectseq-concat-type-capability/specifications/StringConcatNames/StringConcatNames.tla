---- MODULE StringConcatNames ----
EXTENDS Naturals, Sequences
VARIABLE x
Init == x = 0
Next == x' = 1-x
P == []<>(SelectSeq(<<x>>, LAMBDA y: Len("a" \o "a")=2) = <<x>>)
====
