---- MODULE Ord ----
EXTENDS Naturals
CONSTANT M
VARIABLES zeta, alpha, mid
Init == zeta = 0 /\ alpha = "s" /\ mid = [n \in {3, 1} |-> M]
A == zeta < 1 /\ zeta' = zeta + 1 /\ UNCHANGED <<alpha, mid>>
B == zeta = 1 /\ zeta' = 5 /\ UNCHANGED <<alpha, mid>>
Next == A \/ B
====
