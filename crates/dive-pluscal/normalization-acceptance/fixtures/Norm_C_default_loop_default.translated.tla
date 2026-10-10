---- MODULE Norm_C_default_loop_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables x; { L: while (TRUE) { x:=1; }; } } *)
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
