---- MODULE Proc_C_fixed_set_local_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables g=0; process (P \in{1,2}) variables x=0;  { L: x:=x+1; E: g:=x; } } *)
====
