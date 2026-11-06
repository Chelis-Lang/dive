---- MODULE DirectPrintHelperOrdered ----
EXTENDS Naturals, TLC
CONSTANT C
VARIABLE x
Hidden == IF PrintT("review-effect") THEN 5 ELSE 0
Target == Hidden
Init == x=0
Next == x'=IF x<2 THEN x+1 ELSE x
Inv == C+x=x+5 /\ C+x=x+5
====
