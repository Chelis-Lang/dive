---- MODULE Pc_seq_c_current_infinite_loop ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
{variable x=0; { L: while (TRUE) {x:=(x+1)%2;} }}
*)

VARIABLE x

vars == << x >>

Init == /\ x = 0

Next == x' = ( x + 1 ) % 2

Spec == Init /\ [][Next]_vars


====
