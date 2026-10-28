---- MODULE LevelChain ----
EXTENDS Naturals, TLC
VARIABLE x, y
Init == x = 0 /\ y = 0
Next == IF x < 4
        THEN x' = x + 1 /\ y' = TLCGet("level")
        ELSE UNCHANGED <<x, y>>
Inv == y = x
====
