---- MODULE DirectEmptyBase ----
EXTENDS Naturals, TLC
VARIABLE x
Init == x \in SUBSET {}
Next == UNCHANGED x
Inv == x # {}
====
