---- MODULE Norm_C_simple_label_root ----
EXTENDS Naturals, TLC
(* --algorithm A { variables x=0; { x:=1; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "730dcb3f" /\ chksum(tla) = "7e37fc59")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "Step1"

Step1 == /\ pc = "Step1"
         /\ x' = 1
         /\ pc' = "Done"

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Step1
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
