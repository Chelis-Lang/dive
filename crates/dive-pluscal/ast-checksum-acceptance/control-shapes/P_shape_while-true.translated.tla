---- MODULE P_shape_while_true ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x=0; begin L: while TRUE do x:=x+1; end while; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "4e3c0790" /\ chksum(tla) = "8b3b4f22")
VARIABLE x

vars == << x >>

Init == (* Global variables *)
        /\ x = 0

Next == x' = x+1

Spec == Init /\ [][Next]_vars

\* END TRANSLATION 
====
