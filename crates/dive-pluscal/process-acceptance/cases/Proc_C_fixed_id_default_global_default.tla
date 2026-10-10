---- MODULE Proc_C_fixed_id_default_global_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables g; process (P =1) variables x=0;  { L: g:=x; E: skip; } } *)
====
