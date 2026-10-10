---- MODULE MultiBound_C_same_id ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0; process (P =1) variable x=0; { L: x:=1; E: g:=x; } process (Q =1) variable y=0; { M: y:=2; F: g:=y; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "c2f3488b" /\ chksum(tla) = "37f92f94")
VARIABLES pc, g, x, y

vars == << pc, g, x, y >>

ProcSet == {1} \cup {1}

Init == (* Global variables *)
        /\ g = 0
        (* Process P *)
        /\ x = 0
        (* Process Q *)
        /\ y = 0
        /\ pc = [self \in ProcSet |-> CASE self = 1 -> "L"
                                        [] self = 1 -> "M"]

L == /\ pc[1] = "L"
     /\ x' = 1
     /\ pc' = [pc EXCEPT ![1] = "E"]
     /\ UNCHANGED << g, y >>

E == /\ pc[1] = "E"
     /\ g' = x
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ UNCHANGED << x, y >>

P == L \/ E

M == /\ pc[1] = "M"
     /\ y' = 2
     /\ pc' = [pc EXCEPT ![1] = "F"]
     /\ UNCHANGED << g, x >>

F == /\ pc[1] = "F"
     /\ g' = y
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ UNCHANGED << x, y >>

Q == M \/ F

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == P \/ Q
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
