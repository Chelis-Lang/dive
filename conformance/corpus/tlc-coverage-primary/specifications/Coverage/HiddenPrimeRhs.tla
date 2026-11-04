---- MODULE HiddenPrimeRhs ----
EXTENDS Naturals, TLC
VARIABLE x
Init == x=0
Next == x'= CHOOSE i \in 1..1 : IF TRUE THEN TRUE ELSE x'=1
====
