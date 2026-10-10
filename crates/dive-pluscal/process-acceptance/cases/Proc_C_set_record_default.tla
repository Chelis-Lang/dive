---- MODULE Proc_C_set_record_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables g=0; process (P \in{1,2}) { variables x=[a|->0];  L: x.a:=self; E: g:=x.a; } } *)
====
