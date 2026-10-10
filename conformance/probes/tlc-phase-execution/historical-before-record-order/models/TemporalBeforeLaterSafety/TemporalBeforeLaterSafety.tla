---- MODULE TemporalBeforeLaterSafety ----
EXTENDS Integers
VARIABLE x
Init == x \in {0,1}
Next == x < 2 /\ x' = x + 1
Inv == x < 2
Live == []<>(1 \div x = 0)
====
