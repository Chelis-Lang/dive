---- MODULE Proc_C_set_loop_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables g=0; process (P \in{1,2}) { variables x=0;  L: while (x<2) { x:=x+1; }; E: g:=x; } } *)
====
