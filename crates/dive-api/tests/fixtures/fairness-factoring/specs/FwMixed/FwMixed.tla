---- MODULE FwMixed ----
EXTENDS Integers
VARIABLE x
Init == x = [p \in 1..5 |-> FALSE]
Flip(p) == x' = [x EXCEPT ![p] = ~@]
Next == \E p \in 1..5 : Flip(p)
Spec == Init /\ [][Next]_x /\ ((\A p \in 1..2 : SF_x(Flip(p))) /\ (\A p \in 3..5 : WF_x(Flip(p))))
P == \A p \in 1..5 : []<>(x[p])
====
