---- MODULE Proc_C_fixed_id_local_member_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables g=0; process (P =1) variables x\in{0,1};  { L: g:=x; E: x:=0; } } *)
====
