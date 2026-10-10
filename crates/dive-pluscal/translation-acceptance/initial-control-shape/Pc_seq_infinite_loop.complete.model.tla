---- MODULE Pc_seq_infinite_loop ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
variable x=0; begin L: while TRUE do x := (x+1) % 2; end while; end algorithm;
*)

VARIABLE x

vars == << x >>

Init == /\ x = 0

Next == x' = ( x + 1 ) % 2

Spec == Init /\ [][Next]_vars


====
