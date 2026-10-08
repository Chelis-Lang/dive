---- MODULE TmExit ----
EXTENDS Naturals, TLC
VARIABLE x
Init == x \in {0, 10}
Next == /\ x < 20
        /\ IF x = 3 THEN TLCSet("exit", TRUE) ELSE TRUE
        /\ x' \in {x + 1, x + 2}
====
