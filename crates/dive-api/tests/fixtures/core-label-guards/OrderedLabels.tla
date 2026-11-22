---- MODULE OrderedLabels ----
EXTENDS Integers, TLC
VARIABLES t, x
vars == <<t,x>>
Actors == {1,2}
Init == t = [i \in Actors |-> [pc |-> IF i = 1 THEN "idle" ELSE "done",
                                alt |-> "idle"]] /\ x = 0
Advance(i) == /\ t[i].pc = "idle" /\ x' = 1 /\ t' = [t EXCEPT ![i].pc = "done"]
Again(i) == /\ t[i].pc = "idle" /\ x' = 1 /\ t' = [t EXCEPT ![i].pc = "done"]
Other(i) == /\ t[i].alt = "idle" /\ x' = x /\ UNCHANGED t
Off(i) == /\ t[i].pc = "off" /\ t[i].missing = "demanded" /\ UNCHANGED vars
Done(i) == /\ t[i].pc = "done" /\ UNCHANGED vars
Next == \E i \in Actors : Advance(i) \/ Again(i) \/ Other(i) \/ Off(i) \/ Done(i)
Bad(i) == /\ t[i].pc = "idle" /\ t[i].missing = "boom" /\ UNCHANGED vars
ErrorNext == \E i \in Actors : Bad(i) \/ Advance(i) \/ Other(i)
Partial(i) == /\ x' = x /\ t[i].pc = "idle" /\ t' = [t EXCEPT ![i].pc = "done"]
PartialNext == \E i \in Actors : Partial(i) \/ Other(i)
Effects(i) == /\ PrintT("observable") /\ t[i].pc = "off" /\ UNCHANGED vars
EffectNext == \E i \in Actors : Effects(i) \/ Next
Inner(i) == Advance(i) \/ Again(i) \/ Other(i)
Outer(i) == \E j \in Actors : Inner(j) \/ Done(i)
Shadow == \E j \in Actors : Outer(j)
NotAString == t = [i \in Actors |-> [pc |-> 0, alt |-> "idle"]] /\ x = 0
NeverOne == x = 0
====
