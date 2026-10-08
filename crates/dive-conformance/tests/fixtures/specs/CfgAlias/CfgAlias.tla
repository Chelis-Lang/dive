---- MODULE CfgAlias ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = (x + 1) % 2
Alias == [x |-> x, double |-> 2 * x]
====
