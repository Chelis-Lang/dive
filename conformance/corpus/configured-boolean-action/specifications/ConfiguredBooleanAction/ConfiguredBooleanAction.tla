---- MODULE ConfiguredBooleanAction ----
EXTENDS Integers
VARIABLE x
Probe(input) == input = 1
Init == x = 0
Next == \/ /\ Probe(1 \div x) /\ x' = 42
        \/ x' = x
Inv == x = 0
====
