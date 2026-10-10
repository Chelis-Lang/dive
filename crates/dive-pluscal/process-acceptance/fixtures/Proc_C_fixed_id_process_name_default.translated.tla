---- MODULE Proc_C_fixed_id_process_name_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables P=0; process (P =1) variables x=0;  { L: P:=1; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "497ae3e4" /\ chksum(tla) = "fa329323")
\* Process P at line 3 col 35 changed to P_
VARIABLES pc, P, x

vars == << pc, P, x >>

ProcSet == {1}

Init == (* Global variables *)
        /\ P = 0
        (* Process P_ *)
        /\ x = 0
        /\ pc = [self \in ProcSet |-> "L"]

L == /\ pc[1] = "L"
     /\ P' = 1
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ x' = x

P_ == L

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == P_
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
