---- MODULE LaterFalse ----
EXTENDS Naturals
CONSTANT N
ASSUME N > 0
ASSUME N > 3
ASSUME N > 4
VARIABLE x
Init == x = 0
Next == UNCHANGED x
====
