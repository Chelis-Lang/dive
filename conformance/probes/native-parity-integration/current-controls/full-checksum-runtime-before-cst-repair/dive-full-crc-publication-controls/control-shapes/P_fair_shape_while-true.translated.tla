---- MODULE P_fair_shape_while_true ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A variables x=0; begin L: while TRUE do x:=x+1; end while; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "7c28162a" /\ chksum(tla) = "7890923d")
VARIABLE x

vars == << x >>

Init == (* Global variables *)
        /\ x = 0

Next == x' = x+1

Spec == /\ Init /\ [][Next]_vars
        /\ WF_vars(Next)

\* END TRANSLATION 
====
