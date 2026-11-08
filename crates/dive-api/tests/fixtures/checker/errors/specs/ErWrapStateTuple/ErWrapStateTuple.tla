---- MODULE ErWrapStateTuple ----
\* WP-C4 error-class matrix: a tuple held in a state indexed out of range.
EXTENDS Integers, TLC
VARIABLES x, y
Init == x = 0 /\ y = <<1, 2>>
Next == x < 3 /\ x' = x + 1 /\ y' = y /\ (x # 2 \/ y[x + 1] = 1)
====
