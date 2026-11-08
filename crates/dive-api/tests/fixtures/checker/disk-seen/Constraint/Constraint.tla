---- MODULE Constraint ----
EXTENDS Integers
VARIABLE x
Init == x = 0
Next == x' \in {x + 1, x + 2}
Bound == x < 3
Inv == x \in 0..2
====
