---- MODULE GenError ----
EXTENDS Naturals
VARIABLES n, x
Init == n = 0 /\ x = [a |-> 0]
TupleInit == n = 0 /\ x = <<0>>
NestedInit == n = 0 /\ x = [i \in 1..1 |-> [j \in 1..1 |-> [a |-> 0]]]
GeneralInit == n = 0 /\ x = [k \in {"a"} |-> 0]
Next == n' = n + 1 /\ x' = IF n = 0 THEN x ELSE 1 \div 0
P == []<>(n = 0)
====
