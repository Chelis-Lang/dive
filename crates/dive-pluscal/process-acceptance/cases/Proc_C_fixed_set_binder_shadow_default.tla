---- MODULE Proc_C_fixed_set_binder_shadow_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables g=0; process (P \in{1,2}) variables x=0;  { L: with (x=1) { g:=x; }; } } *)
====
