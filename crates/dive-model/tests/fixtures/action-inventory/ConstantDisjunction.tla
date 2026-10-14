---- MODULE ConstantDisjunction ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Never == FALSE \/ FALSE
Step == x' = x
Next == Never \/ Step
====
