---- MODULE FpInfiniteSets ----
\* A successor holding an infinite set TLC cannot fingerprint: Int, inside a tuple.
EXTENDS Integers, Sequences
VARIABLES x, y
Init == x = 0 /\ y = 0
Next == \/ y < 3 /\ x' = x /\ y' = y + 1
        \/ y = 2 /\ x' = <<1, Int>> /\ y' = y
====
