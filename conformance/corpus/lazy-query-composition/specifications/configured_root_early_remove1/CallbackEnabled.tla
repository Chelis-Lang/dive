---- MODULE CallbackEnabled ----
EXTENDS Naturals,Sequences
CONSTANT Act(_)
VARIABLES b,q,s
Init == b=0 /\ q=0 /\ s = <<1,2,3>>
Next == UNCHANGED <<b,q,s>>
Impl(v) == LET Cut == v IN /\ v \in {1,2} /\ s' = SelectSeq(s, LAMBDA e : e /= Cut) /\ s' = <<2,3>>
Inv == IF ENABLED Act(b') THEN TRUE ELSE b/=0
====
