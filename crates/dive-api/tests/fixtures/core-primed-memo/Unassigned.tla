---- MODULE Unassigned ----
\* Size' is read before q' is assigned: the error of the unassigned read.
EXTENDS FiniteSets, Naturals
VARIABLE q
Size == Cardinality(q)
Init == q = {}
Next == /\ Size' < 3
        /\ q' \in {q \cup {n} : n \in 1..3}
====
