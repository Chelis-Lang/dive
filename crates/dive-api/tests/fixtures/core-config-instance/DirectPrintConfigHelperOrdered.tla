---- MODULE DirectPrintConfigHelperOrdered ----
EXTENDS Naturals, TLC
CONSTANT C
CONSTANT D
VARIABLE x
Hidden2 == IF PrintT("review-effect") THEN 5 ELSE 0
Hidden == Hidden2
Target == D
Init == x=0
Next == x'=IF x<2 THEN x+1 ELSE x
Inv == C+x=x+5 /\ C+x=x+5
====
