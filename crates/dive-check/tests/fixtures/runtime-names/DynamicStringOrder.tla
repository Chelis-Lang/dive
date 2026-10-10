---- MODULE DynamicStringOrder ----
EXTENDS Naturals, TLC
VARIABLES x, s
Init == x \in 0..15 /\ s = "unset"
Next == x' = x /\ s' = (CHOOSE t \in {ToString(x), ToString(x+1)} : TRUE)
Inv == x >= 14 \/ s = "unset" \/ s = ToString(x)
====
