---- MODULE Norm_C_minus_loop_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables x=0; { L:- while (TRUE) { x:=1; }; } } *)

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
