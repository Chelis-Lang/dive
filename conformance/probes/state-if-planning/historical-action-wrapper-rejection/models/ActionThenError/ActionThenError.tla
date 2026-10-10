---- MODULE ActionThenError ----
EXTENDS Integers
VARIABLE x
Init == x = 0
Next == x' = (x + 1) % 3
F(c) == IF c THEN 1 \div 0 = 0 ELSE x = 2
Prop == []<>(IF x' = 1 THEN 1 \div 0 = 0 ELSE x = 2)
====
