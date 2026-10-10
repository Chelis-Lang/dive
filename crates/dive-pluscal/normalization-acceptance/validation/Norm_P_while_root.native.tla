---- MODULE Norm_P_while_root ----
EXTENDS Naturals, TLC
(* --algorithm A variables x=0; begin while x<2 do x:=x+1; end while; skip; end algorithm; *)

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
