---- MODULE Proc_C_fixed_id_infinite_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables g=0; process (P =1) variables x=0;  { L: while (TRUE) { x:=(x+1)%2; }; } } *)

VARIABLES g, x

vars == << g, x >>

ProcSet == {1}

Init == /\ g = 0
        /\ x = 0

P == /\ x' = ( x + 1 ) % 2
     /\ g' = g

Next == P

Spec == Init /\ [][Next]_vars


====
