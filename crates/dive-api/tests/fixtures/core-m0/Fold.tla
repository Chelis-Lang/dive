---- MODULE Fold ----
\* A closed initial filter too large to fold within the compile-time budget
\* (R9): 101 * 101 candidate records. It is enumerated at run time instead.
EXTENDS Naturals
CONSTANT M
VARIABLE c
Can == [b : 0..M, w : 0..M]
Few == {x \in 0..10 : x % 2 = 0}
Init == c \in {x \in Can : x.b + x.w \in 1..M}
Next == \/ /\ c.b > 0
           /\ c' = [c EXCEPT !.b = @ - 1]
        \/ /\ c.w > 1
           /\ c' = [c EXCEPT !.w = @ - 2, !.b = @ + 1]
        \/ /\ c.b + c.w <= 1
           /\ UNCHANGED c
Inv == /\ c.b + c.w <= M
       /\ c.w % 2 = 0 \/ c.w \notin Few
Spec == Init /\ [][Next]_c
====
