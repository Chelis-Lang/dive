---- MODULE ProcExtra_C_id_local_member_dependency ----
EXTENDS Naturals,TLC
(* --algorithm A { variables g=0; process (P =1) variables x=0,y\in{x}; { L: g:=y; E: skip; } } *)
====
