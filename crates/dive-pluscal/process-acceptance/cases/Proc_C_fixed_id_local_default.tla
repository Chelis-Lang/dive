---- MODULE Proc_C_fixed_id_local_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables g=0; process (P =1) variables x=0;  { L: x:=x+1; E: g:=x; } } *)
====
