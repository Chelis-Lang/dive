---- MODULE Norm_C_while_root ----
EXTENDS Naturals, TLC
(* --algorithm A { variables x=0; { while (x<2) { x:=x+1; }; skip; } } *)

VARIABLES pc, x

vars == << pc, x >>

Init == /\ x = 0
        /\ pc = "Step1"

Step1 == /\ pc = "Step1"
         /\ IF x < 2
               THEN /\ x' = x + 1
                    /\ pc' = "Step1"
               ELSE /\ TRUE
                    /\ pc' = "Done"
                    /\ x' = x

Terminating == pc = "Done" /\ UNCHANGED vars

Next == Step1 \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

====
