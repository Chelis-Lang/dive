---- MODULE Proc_C_fixed_set_infinite_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables g=0; process (P \in{1,2}) variables x=0;  { L: while (TRUE) { x:=(x+1)%2; }; } } *)
====
