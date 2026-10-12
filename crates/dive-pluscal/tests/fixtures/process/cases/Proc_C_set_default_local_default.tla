---- MODULE Proc_C_set_default_local_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables g=0; process (P \in{1,2}) { variables x;  L: x:=1; E: g:=x; } } *)
====
