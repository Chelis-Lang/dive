---- MODULE CfgRepeatedSection ----
EXTENDS Naturals
CONSTANT N
VARIABLE x
Init == x = 0
Next == x' = (x + 1) % 3
Ok == TRUE
Inv == x < 2
====
