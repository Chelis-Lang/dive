---- MODULE PrAmbig ----
\* With intervals in the specification, an enumerated set of consecutive
\* integers may be TLC's interval: printing it is refused.
EXTENDS Naturals, TLC
VARIABLE x
Init == x \in 1..2
Next == /\ PrintT({x})
        /\ x' = 3 - x
====
