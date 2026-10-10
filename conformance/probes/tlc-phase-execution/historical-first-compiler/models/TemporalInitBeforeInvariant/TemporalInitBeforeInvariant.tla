---- MODULE TemporalInitBeforeInvariant ----
EXTENDS Integers
VARIABLE x
Init == x = 0
Next == x' = x
Inv == FALSE
Live == [](1 \div x = 0)
====
