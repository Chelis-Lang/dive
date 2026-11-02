---- MODULE Generation ----
EXTENDS Integers, FiniteSets
VARIABLES x, y
Init == x = 0 /\ y = 0
Wide == x >= 0 /\ (\E n \in 1..1000 : x' = n /\ y' = y + 1)
Stored == x >= 0 /\ (\E n \in {11, 22, 33} : x' = n /\ y' = y + 1)
Guarded == x >= 0 /\ (\E n \in 1..16 : n = 16 /\ x' = n /\ y' = y + 1)
Constrained == x >= 0 /\ (\E n \in 1..16 : x' = n /\ y' = y + 1)
Constraint == x = 0 \/ x = 16
Partial == x >= 0 /\ (\E n \in 1..16 : x' = n)
Unknown == x >= 0 /\ (\E s \in SUBSET (1..4) : x' = Cardinality(s) /\ y' = y + 1)
LargeTlc == x >= 0 /\ (\E n \in (-2147483647)..2147483647 : x' = n /\ y' = y + 1)
Queried == ENABLED (\E n \in 1..8 : x' = n /\ UNCHANGED y) /\ Wide
Chosen == x' = (CHOOSE n \in {1, 2, 3} : n = 3) /\ y' = y + 1
Universal == (\A n \in 1..4 : TRUE) /\ Wide
Tuple == x >= 0 /\ (\E <<a, b>> \in {<<1, 2>>, <<3, 4>>} : x' = a /\ y' = b)
Multi == x >= 0 /\ (\E a, b \in 1..8 : a = 8 /\ b = 8 /\ x' = a /\ y' = b)
Inv == x >= 0 /\ y >= 0
QuantifiedInv == Inv /\ (\A n \in 1..8 : n > 0) /\ (\E n \in 1..8 : n = 8)
Bad == x = 0
====
