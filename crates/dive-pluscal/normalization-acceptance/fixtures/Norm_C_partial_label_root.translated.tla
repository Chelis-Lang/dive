---- MODULE Norm_C_partial_label_root ----
EXTENDS Naturals, TLC
(* --algorithm A { variables x=0; { x:=1; E: skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "4469c09b" /\ chksum(tla) = "97164769")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "Step1"

Step1 == /\ pc = "Step1"
         /\ x' = 1
         /\ pc' = "E"

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ x' = x

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Step1 \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
