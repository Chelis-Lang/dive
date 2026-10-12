---- MODULE Norm_P_goto_root ----
EXTENDS Naturals, TLC
(* --algorithm A variables x=0; begin goto Done; x:=1; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "75caee8c" /\ chksum(tla) = "fe118b09")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "Step1"

Step1 == /\ pc = "Step1"
         /\ pc' = "Done"
         /\ x' = x

Step2 == /\ pc = "Step2"
         /\ x' = 1
         /\ pc' = "Done"

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Step1 \/ Step2
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
