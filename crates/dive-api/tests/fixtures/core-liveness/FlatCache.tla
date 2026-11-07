---- MODULE FlatCache ----
EXTENDS Naturals
VARIABLE can
Init == can = [black |-> 0, white |-> 1]
Next == can' = [can EXCEPT !.black = IF @ < 1100 THEN @ + 1 ELSE @]
P == []<>(can.white = 1)
Q == <>[](can.black = 1100)
\* Pow keeps these atoms on reference fallback, so rows are still read.
R == []<>(can.white ^ 1 = 1) /\ []<>(can.white ^ 2 = 1)
====
