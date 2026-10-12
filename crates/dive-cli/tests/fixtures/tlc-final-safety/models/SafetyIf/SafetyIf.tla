---- MODULE SafetyIf ----
EXTENDS Naturals
VARIABLE x
Init == x=0
Next == IF x < 2 THEN x' = x+1 ELSE x' = 0
Inv == x<3
====
