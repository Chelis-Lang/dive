---- MODULE ReplayNames ----
EXTENDS Naturals, TLC
VARIABLES x, s
Init == x = 3 + 3 /\ s = "unset"
Next == x' = x /\ s' = (CHOOSE t \in {ToString(x), ToString(x+1)} : TRUE)
Inv == s = "unset" \/ s = ToString(x)
====
