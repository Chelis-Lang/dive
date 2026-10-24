---- MODULE StateCause ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = 1 - x
P == []<>(1 \div (x - x) = 0)
====
