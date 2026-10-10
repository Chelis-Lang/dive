---- MODULE Proc_C_fixed_set_default_global_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables g; process (P \in{1,2}) variables x=0;  { L: g:=x; E: skip; } } *)
====
