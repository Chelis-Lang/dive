---- MODULE Emp ----
\* The empty function has one form: `[k \in {} |-> 1]`, `<<>>` and the
\* member of `[1..0 -> S]` are one value (one distinct state).
EXTENDS Integers, Sequences
VARIABLE x
Init == x \in {[k \in {} |-> 1], <<>>}
Next == UNCHANGED x
Inv == /\ Len(x) = 0
       /\ [k \in {} |-> 1] = <<>>
       /\ x \in [1..0 -> {1}]
====
