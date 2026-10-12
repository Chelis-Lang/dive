---- MODULE Proc_C_fixed_set_shadow_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables x=0; process (P \in{1,2}) variables x=1;  { L: x:=2; E: skip; } } *)
====
