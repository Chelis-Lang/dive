---- MODULE PhaseActionError ----
EXTENDS Integers, TLC
VARIABLE x
Init == x = 0
Next == x' \in {0,1,2}
Live == []<>(IF x' = 1 THEN 1 \div 0 = 0 ELSE x' = 2)
====
