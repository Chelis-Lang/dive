---- MODULE CallbackEnabled ----
EXTENDS Naturals
CONSTANT Act(_)
VARIABLES b,q,s
Svc == INSTANCE Inner WITH a <- b, b <- q, c <- s, Act <- Act
Impl(v) == Svc!Body(v)
Init == b=0 /\ q=0 /\ s = <<1,2,3>>
Next == UNCHANGED <<b,q,s>>
Inv == IF Svc!CanStep THEN TRUE ELSE b /= 0
====
---- MODULE Inner ----
EXTENDS Naturals,Sequences
CONSTANT Act(_)
VARIABLES a,b,c
Body(v) == /\ v \in {1,2} /\ LET Cut == v IN c' = SelectSeq(c, LAMBDA e : e /= Cut) /\ c' = <<1,3>>
Step == /\ b'=42 /\ Act(a')
CanStep == ENABLED Step
====
