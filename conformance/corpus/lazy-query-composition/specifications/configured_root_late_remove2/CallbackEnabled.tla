---- MODULE CallbackEnabled ----
EXTENDS Naturals,Sequences
CONSTANT Act(_)
VARIABLES b,q,s
Init == b=0 /\ q=0 /\ s = <<1,2,3>>
Next == UNCHANGED <<b,q,s>>
Impl(v) == /\ v \in {1,2} /\ LET Cut == v IN s' = SelectSeq(s, LAMBDA e : e /= Cut) /\ s' = <<1,3>>
Inv == IF ENABLED Act(b') THEN TRUE ELSE b/=0
====
