---- MODULE SimAssumeFalse ----
EXTENDS Naturals
VARIABLE x
ASSUME FALSE
Init == x = 0
Next == x' = 1
====
