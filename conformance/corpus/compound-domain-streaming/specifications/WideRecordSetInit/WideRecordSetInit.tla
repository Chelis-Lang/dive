---- MODULE WideRecordSetInit ----
EXTENDS Naturals
VARIABLE x
Init == x \in [a : (1..50000), b : (1..50000)]
Next == UNCHANGED x
Bad == x # x
====
