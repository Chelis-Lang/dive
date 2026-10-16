---- MODULE CallbackEnabled ----
EXTENDS Naturals
VARIABLES b, q, s
Svc == INSTANCE Inner WITH a <- b, b <- q, c <- s
Init == b = 0 /\ q = 0 /\ s = <<1, 2, 3>>
Next == UNCHANGED <<b, q, s>>
Inv == IF Svc!CanStep THEN TRUE ELSE b /= 0
====
---- MODULE Inner ----
EXTENDS Naturals, Sequences
VARIABLES a, b, c
Helper(Cut) == /\ a' \in {1, 2}
                  /\ c' = SelectSeq(c, LAMBDA e : e /= Cut)
                  /\ c' = <<2, 3>>
Step == /\ a = 1 /\ b' = 42 /\ Helper(a')
CanStep == ENABLED Step
====
