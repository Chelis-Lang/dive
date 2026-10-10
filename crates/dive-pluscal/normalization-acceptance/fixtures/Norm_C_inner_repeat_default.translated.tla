---- MODULE Norm_C_inner_repeat_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables x=0; { while (x<2) { x:=x+1; x:=x; }; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "991c8e11" /\ chksum(tla) = "d58bbf38")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "Lbl_1"

Lbl_1 == /\ pc = "Lbl_1"
         /\ IF x<2
               THEN /\ x' = x+1
                    /\ pc' = "Lbl_2"
               ELSE /\ pc' = "Done"
                    /\ x' = x

Lbl_2 == /\ pc = "Lbl_2"
         /\ x' = x
         /\ pc' = "Lbl_1"

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Lbl_1 \/ Lbl_2
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
