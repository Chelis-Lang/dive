---- MODULE ProcExtra_C_set_local_member_dependency ----
EXTENDS Naturals,TLC
(* --algorithm A { variables g=0; process (P \in{1,2}) variables x=0,y\in{x}; { L: g:=y; E: skip; } } *)
====
