---- MODULE Orders ----
\* InnerSerial's shape: Orders' reads only q', so one value serves every ord' candidate.
EXTENDS FiniteSets, Naturals
VARIABLES q, ord
Orders == {R \in SUBSET (q \X q) :
             /\ \A a, b \in q : a = b \/ <<a, b>> \in R \/ <<b, a>> \in R
             /\ \A a \in q : <<a, a>> \notin R}
Top == CHOOSE x \in q : \A y \in q : x >= y
Init == q = {} /\ ord = {}
Next == \/ /\ q' \in {q \cup {n} : n \in 1..2}
           /\ ord' \in SUBSET (q' \X q')
           /\ ord \subseteq ord'
           /\ \E R \in Orders' : ord' \subseteq R
        \/ /\ q' = q
           /\ ord' \in {s \in SUBSET (q' \X q') : FALSE}
           /\ Top' > 0
Inv == ord \subseteq q \X q
====
