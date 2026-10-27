---- MODULE CompatQueue ----
\* Records in sequences, compared and tested for membership: shapes agree.
EXTENDS Naturals, Sequences
Msg == [t : {"req", "ack"}, v : 0..1]
VARIABLE q
Init == q = <<>>
Next == /\ Len(q) < 3
        /\ \E m \in Msg : q' = Append(q, m)
Inv == /\ \A i \in 1..Len(q) : q[i] \in Msg
       /\ q # <<[t |-> "ack", v |-> 1], [t |-> "req", v |-> 0], [t |-> "ack", v |-> 0]>>
====
