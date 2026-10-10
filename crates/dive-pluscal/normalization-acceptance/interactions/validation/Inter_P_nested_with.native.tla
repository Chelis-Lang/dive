---- MODULE Inter_P_nested_with ----
EXTENDS Naturals,TLC
(* --algorithm A variables x=0; begin with k=1 do with j=2 do x:=k+j; end with; end with; end algorithm; *)

VARIABLES pc, x

vars == << pc, x >>

Init == /\ x = 0
        /\ pc = "Lbl_1"

Lbl_1 == /\ pc = "Lbl_1"
         /\ LET k == 1 IN
               LET j == 2 IN
                  x' = k + j
         /\ pc' = "Done"

Terminating == pc = "Done" /\ UNCHANGED vars

Next == Lbl_1 \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

====
