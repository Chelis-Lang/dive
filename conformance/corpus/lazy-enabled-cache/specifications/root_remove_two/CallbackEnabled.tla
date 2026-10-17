---- MODULE CallbackEnabled ----
EXTENDS Naturals, Sequences
VARIABLES a, b, c
Init == a = 0 /\ b = 0 /\ c = <<1, 2, 3>>
Next == UNCHANGED <<a, b, c>>
Step(Cut) == /\ a = 0
             /\ b' = 42
             /\ a' \in {1, 2}
             /\ c' = SelectSeq(c, LAMBDA e : e /= Cut)
             /\ c' = <<1, 3>>
Inv == IF ENABLED Step(a') THEN TRUE ELSE a /= 0
====
