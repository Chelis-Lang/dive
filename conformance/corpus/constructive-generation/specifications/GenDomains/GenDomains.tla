---- MODULE GenDomains ----
EXTENDS Integers
VARIABLE x
Init == x = 0
Next == x >= 0 /\ (\E a \in {}, b \in 1..(1 \div 0) : x' = a + b)
====
