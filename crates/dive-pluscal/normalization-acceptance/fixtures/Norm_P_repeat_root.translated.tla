---- MODULE Norm_P_repeat_root ----
EXTENDS Naturals, TLC
(* --algorithm A variables x=0; begin x:=1; x:=2; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "951b5733" /\ chksum(tla) = "bea73215")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "Step1"

Step1 == /\ pc = "Step1"
         /\ x' = 1
         /\ pc' = "Step2"

Step2 == /\ pc = "Step2"
         /\ x' = 2
         /\ pc' = "Done"

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Step1 \/ Step2
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
