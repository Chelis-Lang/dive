---- MODULE SharedAction ----
EXTENDS Naturals, TLC
VARIABLE x
Init == x=0
A(n) == x'=n
Next == A(1) \/ A(2)
====
