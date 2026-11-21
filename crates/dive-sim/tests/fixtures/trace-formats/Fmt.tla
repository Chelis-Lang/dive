---- MODULE Fmt ----
EXTENDS Naturals, Sequences
VARIABLES x, s, f
Init == x = 0 /\ s = <<>> /\ f = [i \in {"a", "b"} |-> {1, 2}]
Inc == x' = x + 1 /\ s' = Append(s, x) /\ UNCHANGED f
Next == Inc
Inv == x < 3
Al == [x |-> x, doubled |-> 2 * x]
====
