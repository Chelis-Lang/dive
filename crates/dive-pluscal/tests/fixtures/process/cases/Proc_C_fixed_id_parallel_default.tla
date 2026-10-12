---- MODULE Proc_C_fixed_id_parallel_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables g=0; process (P =1) variables x=0,y=0;  { L: x:=y+1 || y:=x+1; E: g:=x+y; } } *)
====
