---- MODULE RgAll ----
\* TLCGet("all") and TLCGet("all:named"): every register, as TLC's
\* function from register to its one worker's value.
EXTENDS Naturals, TLC
VARIABLE x
ASSUME TLCSet(2, "a") /\ TLCSet("s:k", 3)
Init == x = 0
Next == \/ /\ x < 2
           /\ PrintT(TLCGet("all"))
           /\ PrintT(TLCGet("all:named"))
           /\ TLCSet(x + 3, x)
           /\ x' = x + 1
        \/ /\ x = 2
           /\ UNCHANGED x
====
