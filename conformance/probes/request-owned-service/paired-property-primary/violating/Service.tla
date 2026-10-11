---- MODULE Service ----
EXTENDS Integers
VARIABLE x
Init == x \in 0..1
Next == x' \in 0..1
Live == []<>(x=0)
====
