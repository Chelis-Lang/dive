---- MODULE LazyActionEnabled ----
EXTENDS Integers
VARIABLE x
Init==x=0
Next==x'=1
Guarded(g)==g /\ ENABLED (x'=7 /\ ENABLED (x'=8))
Ordered(g)==ENABLED (x'=7 /\ 1 \div 0=0) /\ g
A==Guarded(x'=1)
B==Guarded(FALSE /\ 1 \div 0=0 /\ x'=1)
C==Ordered(FALSE /\ x'=1)
====
