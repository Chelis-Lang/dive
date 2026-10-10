---- MODULE Proc_C_set_if_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables g=0; process (P \in{1,2}) { variables x=0;  L: if (x=0) { x:=1; } else { g:=2; }; E: skip; } } *)
====
