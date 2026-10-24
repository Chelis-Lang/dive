---- MODULE RightChildErrorBeforeKernel ----
EXTENDS Naturals, Sequences
VARIABLE x
Init == x = 0
Next == x' = 1-x
P == []<>(SelectSeq(<<x>>, LAMBDA y: Len(TRUE \o <<1 \div (x-x)>>)=0) = <<>>)
====
