---- MODULE Inter_C_with_after ----
EXTENDS Naturals,TLC
(* --algorithm A { variables x=0; { with (k=1) { x:=k; }; x:=2; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "47c3c69f" /\ chksum(tla) = "f4e14c11")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "Lbl_1"

Lbl_1 == /\ pc = "Lbl_1"
         /\ LET k == 1 IN
              x' = k
         /\ pc' = "Lbl_2"

Lbl_2 == /\ pc = "Lbl_2"
         /\ x' = 2
         /\ pc' = "Done"

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Lbl_1 \/ Lbl_2
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
