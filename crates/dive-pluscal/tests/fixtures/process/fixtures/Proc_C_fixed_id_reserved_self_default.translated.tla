---- MODULE Proc_C_fixed_id_reserved_self_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables g=0; process (P =1) variables self=0;  { L: self:=1; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "e93b886b" /\ chksum(tla) = "94592a02")
VARIABLES pc, g, self

vars == << pc, g, self >>

ProcSet == {1}

Init == (* Global variables *)
        /\ g = 0
        (* Process P *)
        /\ self = 0
        /\ pc = [self \in ProcSet |-> "L"]

L == /\ pc[1] = "L"
     /\ self' = 1
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ g' = g

P == L

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == P
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
