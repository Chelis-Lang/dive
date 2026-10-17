---- MODULE CallbackEnabled ----
EXTENDS Naturals
VARIABLES b, q, s
Svc == INSTANCE Inner WITH a <- b, b <- q, c <- s
Init == b = 0 /\ q = 0 /\ s = <<1, 2, 3>>
Next == UNCHANGED <<b, q, s>>
Inv == IF Svc!CanStep THEN b /= 0 ELSE TRUE
====
---- MODULE Inner ----
EXTENDS Naturals, Sequences
VARIABLES a, b, c
Step == /\ a = 0 /\ b' = 42
        /\ LET Cut == a' IN /\ a' \in {1, 2}
                            /\ c' = SelectSeq(c, LAMBDA e : e /= Cut)
                  /\ c' = <<1, 3>>
CanStep == ENABLED Step
====
