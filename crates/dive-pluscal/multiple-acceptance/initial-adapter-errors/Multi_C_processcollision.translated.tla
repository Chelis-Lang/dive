---- MODULE Multi_C_processcollision ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0;  process (P =1) variable Q=0; { L: Q:=1; }  process (Q =2) variable y=0; { M: y:=2; F: g:=y; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "868a8cab" /\ chksum(tla) = "c750b0db")
\* Process Q at line 3 col 78 changed to Q_
VARIABLES pc, g, Q, y

vars == << pc, g, Q, y >>

ProcSet == {1} \cup {2}

Init == (* Global variables *)
        /\ g = 0
        (* Process P *)
        /\ Q = 0
        (* Process Q_ *)
        /\ y = 0
        /\ pc = [self \in ProcSet |-> CASE self = 1 -> "L"
                                        [] self = 2 -> "M"]

L == /\ pc[1] = "L"
     /\ Q' = 1
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ UNCHANGED << g, y >>

P == L

M == /\ pc[2] = "M"
     /\ y' = 2
     /\ pc' = [pc EXCEPT ![2] = "F"]
     /\ UNCHANGED << g, Q >>

F == /\ pc[2] = "F"
     /\ g' = y
     /\ pc' = [pc EXCEPT ![2] = "Done"]
     /\ UNCHANGED << Q, y >>

Q_ == M \/ F

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == P \/ Q_
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
