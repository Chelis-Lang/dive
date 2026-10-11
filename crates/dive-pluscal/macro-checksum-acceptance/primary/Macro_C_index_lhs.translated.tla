---- MODULE Macro_C_index_lhs ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=[i \in 1..2 |-> 0], y=0; macro Put(v, a) { v := a; }; { L: Put(x[1],2); E: skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "4d586894" /\ chksum(tla) = "fee4f784")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = [i \in 1..2 |-> 0]
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = [x EXCEPT ![1] = 2]
     /\ pc' = "E"
     /\ y' = y

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, y >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
