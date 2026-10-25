---- MODULE BaseSizeOverflow ----
EXTENDS Naturals, TLC
VARIABLE x
Init == x \in SUBSET ({1, 2} \X (1..1073741824))
Next == UNCHANGED x
Inv == x # {}
====
