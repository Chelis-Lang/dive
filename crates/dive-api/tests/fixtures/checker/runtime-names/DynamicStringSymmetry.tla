---- MODULE DynamicStringSymmetry ----
EXTENDS Naturals, TLC
CONSTANTS a, b
VARIABLES x, s, q
Syms == Permutations({a,b})
Init == q = a /\ x \in 0..15 /\ s = "unset"
Next == q' = q /\ x' = x /\ s' = (CHOOSE t \in {ToString(x), ToString(x+1)} : TRUE)
Inv == x >= 14 \/ s = "unset" \/ s = ToString(x)
====
