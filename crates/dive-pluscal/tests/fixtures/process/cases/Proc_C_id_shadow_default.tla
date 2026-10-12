---- MODULE Proc_C_id_shadow_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables x=0; process (P =1) { variables x=1;  L: x:=2; E: skip; } } *)
====
