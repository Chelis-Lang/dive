---- MODULE Norm_C_with_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables x=0; { with (k=1) { x:=k; }; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "5db22b61" /\ chksum(tla) = "38b3ff4f")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "Lbl_1"

Lbl_1 == /\ pc = "Lbl_1"
         /\ LET k == 1 IN
              x' = k
         /\ pc' = "Done"

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Lbl_1
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
