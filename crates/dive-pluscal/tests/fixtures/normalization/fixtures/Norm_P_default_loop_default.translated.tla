---- MODULE Norm_P_default_loop_default ----
EXTENDS Naturals, TLC
(* --algorithm A variables x; begin L: while TRUE do x:=1; end while; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "e17065fa" /\ chksum(tla) = "53c1529e")
CONSTANT defaultInitValue
VARIABLE x

vars == << x >>

Init == (* Global variables *)
        /\ x = defaultInitValue

Next == x' = 1

Spec == Init /\ [][Next]_vars

\* END TRANSLATION 
====
