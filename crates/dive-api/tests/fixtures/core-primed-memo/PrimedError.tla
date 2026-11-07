---- MODULE PrimedError ----
\* Big' fails once q' has no element above 2: the error is raised, never cached.
EXTENDS FiniteSets, Naturals
VARIABLES q, ord
Big == CHOOSE x \in q : x > 2
Init == q = {3} /\ ord = {}
Next == /\ q' \in {q \cup {n} : n \in 1..3} \cup {q \ {3}}
        /\ ord' \in SUBSET (q' \X q')
        /\ Big' \in q'
====
