---- MODULE WideFunctionSetInit ----
EXTENDS Naturals
VARIABLE x
Init == x \in [(1..2) -> (1..50000)]
Next == UNCHANGED x
Bad == x # x
====
