---- MODULE Norm_P_partial_label ----
EXTENDS Naturals, TLC
(* --algorithm A variables x=0; begin x:=1; E: skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "2eccac9b" /\ chksum(tla) = "171ebd3f")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "Lbl_1"

Lbl_1 == /\ pc = "Lbl_1"
         /\ x' = 1
         /\ pc' = "E"

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ x' = x

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Lbl_1 \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
