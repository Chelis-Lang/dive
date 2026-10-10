---- MODULE TemporalTableauInitBeforeLaterRoots ----
EXTENDS Integers
VARIABLE x
Init == x \in {0,1}
Next == x' = x
Inv == TRUE
Live == [](1 \div x = 0 => <>(x = 2))
====
