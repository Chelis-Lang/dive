---- MODULE FailDemand ----
EXTENDS Naturals, TLC
VARIABLE x
Init == x=0
Next == x'= CHOOSE i \in 1..3 : Assert(FALSE,"demand")
====
