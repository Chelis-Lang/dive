---- MODULE Assumption ----
EXTENDS Naturals
CONSTANT N
ASSUME N > 10
VARIABLE x
Init == x = 0
Next == UNCHANGED x
====
