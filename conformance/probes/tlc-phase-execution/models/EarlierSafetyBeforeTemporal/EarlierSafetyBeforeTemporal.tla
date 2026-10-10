---- MODULE EarlierSafetyBeforeTemporal ----
EXTENDS Integers
VARIABLE x
Init == x = 0
Next == x' = 1
Inv == x = 0
Live == []<>(1 \div x = 0)
====
