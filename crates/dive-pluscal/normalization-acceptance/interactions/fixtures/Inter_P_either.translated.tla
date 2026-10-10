---- MODULE Inter_P_either ----
EXTENDS Naturals,TLC
(* --algorithm A variables x=0,y=0; begin either x:=1; or y:=1; end either; x:=2; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "61cd6181" /\ chksum(tla) = "da3d4c11")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "Lbl_1"

Lbl_1 == /\ pc = "Lbl_1"
         /\ \/ /\ x' = 1
               /\ y' = y
            \/ /\ y' = 1
               /\ x' = x
         /\ pc' = "Lbl_2"

Lbl_2 == /\ pc = "Lbl_2"
         /\ x' = 2
         /\ pc' = "Done"
         /\ y' = y

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Lbl_1 \/ Lbl_2
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
