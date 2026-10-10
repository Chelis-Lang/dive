---- MODULE Norm_P_plus_loop_default ----
EXTENDS Naturals, TLC
(* --algorithm A variables x=0; begin L:+ while TRUE do x:=1; end while; end algorithm; *)

VARIABLES pc, x

vars == << pc, x >>

Init == /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = 1
     /\ pc' = "L"

Next == L

Spec == Init /\ [][Next]_vars


====
