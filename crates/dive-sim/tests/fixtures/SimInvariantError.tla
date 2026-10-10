---- MODULE SimInvariantError ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = 1
Inv == IF x = 0 THEN TRUE ELSE 1 \div 0 = 0
====
