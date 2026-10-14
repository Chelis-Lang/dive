---- MODULE DirectTrue ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Step == x' = x
Next == TRUE \/ Step
====
