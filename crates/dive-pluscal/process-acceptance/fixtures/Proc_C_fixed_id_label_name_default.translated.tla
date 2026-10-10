---- MODULE Proc_C_fixed_id_label_name_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables g=0; process (P =1) variables x=0;  { P: x:=1; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "31bc00b8" /\ chksum(tla) = "6e805523")
\* Label P of process P at line 3 col 71 changed to P_
VARIABLES pc, g, x

vars == << pc, g, x >>

ProcSet == {1}

Init == (* Global variables *)
        /\ g = 0
        (* Process P *)
        /\ x = 0
        /\ pc = [self \in ProcSet |-> "P_"]

P_ == /\ pc[1] = "P_"
      /\ x' = 1
      /\ pc' = [pc EXCEPT ![1] = "Done"]
      /\ g' = g

P == P_

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == P
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
