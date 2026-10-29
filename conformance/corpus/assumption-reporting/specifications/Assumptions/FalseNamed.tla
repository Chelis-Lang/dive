---- MODULE FalseNamed ----
EXTENDS Naturals
CONSTANT N
ASSUME TooSmall == N > 3
VARIABLE x
Init == x = 0
Next == UNCHANGED x
====
