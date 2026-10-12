---- MODULE Proc_C_fixed_set_with_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables g=0; process (P \in{1,2}) variables x=0;  { L: with (k\in{0,1}) { x:=k; }; E: g:=x; } } *)
====
