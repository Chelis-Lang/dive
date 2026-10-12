---- MODULE Proc_C_fixed_id_loop_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables g=0; process (P =1) variables x=0;  { L: while (x<2) { x:=x+1; }; E: g:=x; } } *)
====
