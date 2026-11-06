---- MODULE Controls ----
EXTENDS Integers, TLC
VARIABLES x, y
Init == x = 0 /\ y = 0
State == x + 1
First == x' \in {1, 2} /\ y' = x'
Reduction == LET m == x IN x' = x - m /\ y' = y + m
Basic == First \cdot Reduction
Duplicates == (First \/ First) \cdot Reduction
Scoped == \E n \in {1, 2} :
    LET q == State IN
    (x' = n /\ y' = q) \cdot (x' = State /\ y' = q)
AllAssigned == x' = 0 /\ y' = 2 /\ (First \cdot Reduction)
Incomplete == (x' = 1) \cdot Reduction
Nondeterministic == First \cdot (x' \in {0, 1} /\ y' = 0)
Pending == (First \cdot Reduction) /\ TRUE
Preassigned == x' = 0 /\ (First \cdot Reduction)
Effect == (PrintT(TRUE) /\ First) \cdot Reduction
Nested == (First \cdot Reduction) \cdot Reduction
PrimedTail == First \cdot (x' = 0 /\ y' = x')
RepeatedTail == First \cdot (x' = 0 /\ x' = 0 /\ y' = 0)
Inv == x = 0 /\ y \in {0, 2, 4}
Bad == y # 4
Other == y # 2
ScopedInv == x = y /\ x \in {0, 2, 3}
AllInv == x = 0 /\ y \in {0, 2}
====
