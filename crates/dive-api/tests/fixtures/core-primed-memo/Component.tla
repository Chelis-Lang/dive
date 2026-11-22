---- MODULE Component ----
VARIABLES q, noise, table
Init == q = 0 /\ noise = 0 /\ table = [i \in {0,1} |-> 0]
Next == /\ q' \in {0,1}
        /\ noise' \in {0,1,2}
        /\ table' = [i \in {0,1} |-> IF i = q THEN q' ELSE q]
====
