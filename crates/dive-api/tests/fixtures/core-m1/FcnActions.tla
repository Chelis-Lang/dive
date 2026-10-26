---- MODULE FcnActions ----
\* Function constructors applied as actions (getFcnContext): one parameter,
\* two parameters, a tuple pattern, and an argument outside the domain
\* guarded away.
EXTENDS Naturals
VARIABLES x, y
Init == x = 0 /\ y = 0
One[i \in 1..2] == x < 3 /\ x' = x + i /\ y' = y
Two[i \in 1..2, j \in {0, 1}] == y < 2 /\ y' = y + j /\ x' = (x + i) % 4
Pair == [<<i, j>> \in {<<1, 0>>, <<0, 1>>} |-> x' = (x + i) % 4 /\ y' = (y + j) % 3]
Next == \/ \E i \in 1..2 : One[i]
        \/ \E i \in 1..2, j \in {0, 1} : Two[i, j]
        \/ \E p \in {<<1, 0>>, <<0, 1>>} : Pair[p]
====
