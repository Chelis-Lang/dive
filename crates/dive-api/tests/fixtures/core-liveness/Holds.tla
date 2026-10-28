---- MODULE Holds ----
\* Leads-to and recurrence that hold under strong fairness, over records.
EXTENDS Naturals, Sequences
VARIABLES r, n
Init == r = [a |-> 0, s |-> <<>>] /\ n \in {0, 1}
Step == r' = [r EXCEPT !.a = (@ + 1) % 4] /\ n' = n
Log == Len(r.s) < 2 /\ r' = [r EXCEPT !.s = Append(@, n)] /\ n' = 1 - n
Spec == Init /\ [][Step \/ Log]_<<r, n>> /\ SF_<<r, n>>(Step)
P == (r.a = 1) ~> (r.a = 2)
Q == []<>(r.a = 0)
====
