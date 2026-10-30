---- MODULE ClosedFunctions ----
EXTENDS Naturals, Sequences
VARIABLES active, counter, color, inbox
vars == <<active, counter, color, inbox>>
Node == 0..2
Init == /\ active = [i \in Node |-> TRUE]
        /\ counter = [i \in Node |-> 0]
        /\ color = [i \in Node |-> "white"]
        /\ inbox = [i \in Node |-> <<[type |-> "tok", q |-> 0]>>]
Next == /\ counter[0] < 1100
        /\ counter' = [counter EXCEPT ![0] = @ + 1]
        /\ active' = [active EXCEPT ![0] = ~@]
        /\ color' = [color EXCEPT ![0] = IF @ = "white" THEN "black" ELSE "white"]
        /\ inbox' = [inbox EXCEPT ![0] = IF active[0]
                        THEN <<[type |-> "pl", q |-> 1]>>
                        ELSE <<[type |-> "tok", q |-> 0]>>]
H == []<> (\A i \in Node : Len(SelectSeq(inbox[i], LAMBDA msg: msg.type = "pl")) <= 1)
P == []<> (counter[0] = 0)
====
