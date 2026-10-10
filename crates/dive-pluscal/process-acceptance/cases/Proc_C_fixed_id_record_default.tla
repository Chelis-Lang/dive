---- MODULE Proc_C_fixed_id_record_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables g=0; process (P =1) variables x=[a|->0];  { L: x.a:=self; E: g:=x.a; } } *)
====
