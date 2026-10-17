---- MODULE CallbackEnabled ----
EXTENDS Naturals
VARIABLES b,q,s
Svc == INSTANCE Inner WITH a <- b, b <- q, c <- s
Init == b=0 /\ q=0 /\ s = <<1,2,3>>
Next == UNCHANGED <<b,q,s>>
Inv == IF Svc!CanStep THEN TRUE ELSE b /= 0
====
---- MODULE Inner ----
EXTENDS Naturals, Sequences
VARIABLES a,b,c
Step == /\ a=0 /\ b'=42 /\ (LET f[t \in {0,1}] == IF t=0 THEN {1,2} ELSE f[0] Cut == a' IN /\ a' \in f[1] /\ c' = SelectSeq(c, LAMBDA e : e /= Cut) /\ c' = <<2,3>>)
CanStep == ENABLED Step
====
