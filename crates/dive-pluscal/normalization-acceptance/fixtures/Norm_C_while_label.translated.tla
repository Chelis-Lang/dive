---- MODULE Norm_C_while_label ----
EXTENDS Naturals, TLC
(* --algorithm A { variables x=0; { while (x<2) { x:=x+1; }; skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "1ba8f29b" /\ chksum(tla) = "64d4c345")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "Lbl_1"

Lbl_1 == /\ pc = "Lbl_1"
         /\ IF x<2
               THEN /\ x' = x+1
                    /\ pc' = "Lbl_1"
               ELSE /\ TRUE
                    /\ pc' = "Done"
                    /\ x' = x

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Lbl_1
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
