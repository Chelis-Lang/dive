---- MODULE TmExitInit ----
EXTENDS Naturals, TLC
VARIABLE x
Init == x \in {0, 1} /\ TLCSet("exit", TRUE)
Next == x < 5 /\ x' = x + 1
====
