---- MODULE DirectFalse ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Step == x' = x
Next == FALSE \/ FALSE \/ Step
====
