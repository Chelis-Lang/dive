---- MODULE TemporalTableauInitBeforeInvariant ----
EXTENDS Integers
VARIABLE x
Init == x = 0
Next == x' = x
Inv == x < 0
Live == [](1 \div x = 0 => <>(x = 2))
====
