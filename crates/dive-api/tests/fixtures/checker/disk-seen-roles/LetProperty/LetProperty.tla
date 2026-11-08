---- MODULE LetProperty ----
EXTENDS Naturals, TLC
VARIABLE x
Init == x = 0
Next == x < 3 /\ x' = x + 1
P == LET Safe == x < 3 IN []Safe
====
