---- MODULE RoConstraints ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = x + 1
Bound == x < 3
Step == x' > x
View == x % 2
Alias == [x |-> x, double |-> 2 * x]
Post == TRUE
Inv == x < 10
====
