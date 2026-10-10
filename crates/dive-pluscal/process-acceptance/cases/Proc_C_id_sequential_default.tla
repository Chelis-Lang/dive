---- MODULE Proc_C_id_sequential_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables g=0; process (P =1) { variables x=0;  L: x:=1; g:=x; E: skip; } } *)
====
