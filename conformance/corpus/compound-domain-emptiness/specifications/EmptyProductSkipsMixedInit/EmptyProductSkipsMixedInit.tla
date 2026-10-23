---- MODULE EmptyProductSkipsMixedInit ----
EXTENDS Naturals
VARIABLE x
Init == (x \in ({1, TRUE} \X {})) \/ (x = 0)
Next == UNCHANGED x
Bad == x # x
====
