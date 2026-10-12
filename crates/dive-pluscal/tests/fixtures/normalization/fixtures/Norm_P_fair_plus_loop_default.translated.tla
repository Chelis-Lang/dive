---- MODULE Norm_P_fair_plus_loop_default ----
EXTENDS Naturals, TLC
(* --fair algorithm A variables x=0; begin L:+ while TRUE do x:=1; end while; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "7c28162a" /\ chksum(tla) = "80cc66ed")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = 1
     /\ pc' = "L"

Next == L

Spec == /\ Init /\ [][Next]_vars
        /\ WF_vars(Next)

\* END TRANSLATION 
====
