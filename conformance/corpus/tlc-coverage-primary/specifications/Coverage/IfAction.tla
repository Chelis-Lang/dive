---- MODULE IfAction ----
EXTENDS Naturals, TLC
VARIABLE x
Init == x=0
Next == IF x<2 THEN x'=x+1 ELSE x'=x
====
