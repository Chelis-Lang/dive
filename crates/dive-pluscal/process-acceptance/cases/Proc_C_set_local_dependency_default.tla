---- MODULE Proc_C_set_local_dependency_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables g=0; process (P \in{1,2}) { variables x=0,y=x+1;  L: g:=y; E: x:=g; } } *)
====
