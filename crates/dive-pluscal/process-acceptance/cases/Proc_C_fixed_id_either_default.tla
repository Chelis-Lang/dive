---- MODULE Proc_C_fixed_id_either_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables g=0; process (P =1) variables x=0;  { L: either { x:=1; } or { g:=2; }; E: skip; } } *)
====
