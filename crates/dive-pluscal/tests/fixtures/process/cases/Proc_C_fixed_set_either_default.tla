---- MODULE Proc_C_fixed_set_either_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables g=0; process (P \in{1,2}) variables x=0;  { L: either { x:=1; } or { g:=2; }; E: skip; } } *)
====
