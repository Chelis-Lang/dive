---- MODULE Norm_C_default_loop_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables x; { L: while (TRUE) { x:=1; }; } } *)

CONSTANT defaultInitValue

VARIABLE x

vars == << x >>

Init == /\ x = defaultInitValue

Next == x' = 1

Spec == Init /\ [][Next]_vars


====
