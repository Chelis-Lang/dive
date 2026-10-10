---- MODULE Proc_C_fixed_id_self_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables g=0; process (P =1) variables x=self;  { L: g:=x; E: x:=self+1; } } *)
====
