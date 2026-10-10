---- MODULE FairPositive_C_id_strong_minus_pair ----
EXTENDS Naturals,TLC
(* --algorithm A { variables g=0; fair + process (P =1) variables x=0; { L: - x:=1; E: - g:=x; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "f600cec8" /\ chksum(tla) = "602aab73")
VARIABLES pc, g, x

vars == << pc, g, x >>

ProcSet == {1}

Init == (* Global variables *)
        /\ g = 0
        (* Process P *)
        /\ x = 0
        /\ pc = [self \in ProcSet |-> "L"]

L == /\ pc[1] = "L"
     /\ x' = 1
     /\ pc' = [pc EXCEPT ![1] = "E"]
     /\ g' = g

E == /\ pc[1] = "E"
     /\ g' = x
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ x' = x

P == L \/ E

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == P
           \/ Terminating

Spec == /\ Init /\ [][Next]_vars
        /\ SF_vars((pc[1] \notin {"L", "E"}) /\ P)

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
