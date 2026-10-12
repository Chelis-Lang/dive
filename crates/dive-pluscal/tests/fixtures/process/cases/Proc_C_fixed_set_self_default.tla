---- MODULE Proc_C_fixed_set_self_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables g=0; process (P \in{1,2}) variables x=self;  { L: g:=x; E: x:=self+1; } } *)
====
