---- MODULE StateGuardNegated ----
EXTENDS Integers
VARIABLE x
Init == x = 0
Next == x' = (x + 1) % 3
F(c) == IF c THEN 1 \div 0 = 0 ELSE x = 2
Prop == []<>(IF ~(x = 1 /\ 1 \div 0 = 0) THEN x = 2 ELSE x = 0)
====
