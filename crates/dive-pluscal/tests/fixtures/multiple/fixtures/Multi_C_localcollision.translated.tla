---- MODULE Multi_C_localcollision ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0;  process (P =1) variable x=0; { L: x:=1; E: g:=x; }  process (Q =2) variable x=0; { M: x:=2; F: g:=x; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "eb20ae2b" /\ chksum(tla) = "84d5a6d6")
\* Process variable x of process P at line 3 col 59 changed to x_
VARIABLES pc, g, x_, x

vars == << pc, g, x_, x >>

ProcSet == {1} \cup {2}

Init == (* Global variables *)
        /\ g = 0
        (* Process P *)
        /\ x_ = 0
        (* Process Q *)
        /\ x = 0
        /\ pc = [self \in ProcSet |-> CASE self = 1 -> "L"
                                        [] self = 2 -> "M"]

L == /\ pc[1] = "L"
     /\ x_' = 1
     /\ pc' = [pc EXCEPT ![1] = "E"]
     /\ UNCHANGED << g, x >>

E == /\ pc[1] = "E"
     /\ g' = x_
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ UNCHANGED << x_, x >>

P == L \/ E

M == /\ pc[2] = "M"
     /\ x' = 2
     /\ pc' = [pc EXCEPT ![2] = "F"]
     /\ UNCHANGED << g, x_ >>

F == /\ pc[2] = "F"
     /\ g' = x
     /\ pc' = [pc EXCEPT ![2] = "Done"]
     /\ UNCHANGED << x_, x >>

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
