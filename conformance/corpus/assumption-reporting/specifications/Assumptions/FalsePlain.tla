---- MODULE FalsePlain ----
EXTENDS Naturals
CONSTANT N
ASSUME N > 3
VARIABLE x
Init == x = 0
Next == UNCHANGED x
====
