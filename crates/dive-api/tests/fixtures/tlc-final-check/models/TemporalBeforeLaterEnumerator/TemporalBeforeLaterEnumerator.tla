---- MODULE TemporalBeforeLaterEnumerator ----
EXTENDS Integers
VARIABLE x
Init == x \in {0,1}
Next == x' = IF x = 0 THEN 0 ELSE 1 \div 0
Inv == TRUE
Live == []<>(1 \div x = 0)
====
