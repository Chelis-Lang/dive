---- MODULE WideSubsetInit ----
EXTENDS Naturals
VARIABLE x
Init == x \in SUBSET (1..31)
Next == UNCHANGED x
Bad == x # x
====
