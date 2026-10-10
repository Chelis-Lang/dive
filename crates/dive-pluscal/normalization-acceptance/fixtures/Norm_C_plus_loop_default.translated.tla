---- MODULE Norm_C_plus_loop_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables x=0; { L:+ while (TRUE) { x:=1; }; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "379e94ee" /\ chksum(tla) = "596890e3")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = 1
     /\ pc' = "L"

Next == L

Spec == Init /\ [][Next]_vars

\* END TRANSLATION 
====
