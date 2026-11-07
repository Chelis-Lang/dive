---- MODULE FlatCache ----
EXTENDS Naturals, TLC
VARIABLE can
Init == can = [black |-> 0, white |-> 1]
Next == can' = [can EXCEPT !.black = IF @ < 1100 THEN @ + 1 ELSE @]
P == []<>(can.white = 1)
Q == <>[](can.black = 1100)
\* ToString keeps these atoms on reference fallback, so rows are still read.
R == []<>(ToString(can.white) = "1") /\ []<>(ToString(can.white * 2) = "2")
====
