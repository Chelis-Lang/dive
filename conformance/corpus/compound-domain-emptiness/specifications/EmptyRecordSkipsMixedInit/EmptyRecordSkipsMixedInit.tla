---- MODULE EmptyRecordSkipsMixedInit ----
EXTENDS Naturals
VARIABLE x
Init == (x \in [a : {1, TRUE}, b : {}]) \/ (x = 0)
Next == UNCHANGED x
Bad == x # x
====
