---- MODULE Proc_C_id_default_local_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables g=0; process (P =1) { variables x;  L: x:=1; E: g:=x; } } *)
====
