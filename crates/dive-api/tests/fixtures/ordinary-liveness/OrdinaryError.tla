---- MODULE OrdinaryError ----
EXTENDS Naturals
VARIABLE x
Init == x=0
Next == x'= (x+1)%5
P == []<>(1 \div x = 1)
====
