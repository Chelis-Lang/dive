---- MODULE WideIntervalNext ----
EXTENDS Integers
VARIABLE x
Init == x \in {-1}
Next == x' \in 0..1000000000
Inv == x = -1
====
