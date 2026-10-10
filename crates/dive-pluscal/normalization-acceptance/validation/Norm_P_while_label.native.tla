---- MODULE Norm_P_while_label ----
EXTENDS Naturals, TLC
(* --algorithm A variables x=0; begin while x<2 do x:=x+1; end while; skip; end algorithm; *)

VARIABLES pc, x

vars == << pc, x >>

Init == /\ x = 0
        /\ pc = "Lbl_1"

Lbl_1 == /\ pc = "Lbl_1"
         /\ IF x < 2
               THEN /\ x' = x + 1
                    /\ pc' = "Lbl_1"
               ELSE /\ TRUE
                    /\ pc' = "Done"
                    /\ x' = x

Terminating == pc = "Done" /\ UNCHANGED vars

Next == Lbl_1 \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

====
