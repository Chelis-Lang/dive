---- MODULE Norm_P_default_loop_default ----
EXTENDS Naturals, TLC
(* --algorithm A variables x; begin L: while TRUE do x:=1; end while; end algorithm; *)

CONSTANT defaultInitValue

VARIABLE x

vars == << x >>

Init == /\ x = defaultInitValue

Next == x' = 1

Spec == Init /\ [][Next]_vars


====
