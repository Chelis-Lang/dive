---- MODULE Proc_C_fixed_id_binder_shadow_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables g=0; process (P =1) variables x=0;  { L: with (x=1) { g:=x; }; } } *)
====
