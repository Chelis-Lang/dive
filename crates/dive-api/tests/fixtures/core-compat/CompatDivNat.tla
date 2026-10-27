---- MODULE CompatDivNat ----
\* \div from Naturals alone is TLC's Naturals.Divide.
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = IF x > 0 - 9 THEN (x - 7) \div 2 ELSE x
Inv == x # 0 - 6
====
