---- MODULE TupleStorage ----
EXTENDS Naturals
VARIABLES n,s
Init == n = 0 /\ s = [i \in 1..1 |-> 1]
Next == n' = (IF s = [a |-> 1] THEN 1 ELSE 2) /\ UNCHANGED s
====
