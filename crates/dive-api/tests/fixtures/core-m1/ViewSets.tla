---- MODULE ViewSets ----
\* VIEW over sets and records; a deadlock once s is full.
EXTENDS Naturals
VARIABLES s, n
Init == s = {} /\ n = 0
Next == \E e \in 1..3 \ s : s' = s \cup {e} /\ n' = n + e
View == [k |-> s]
====
