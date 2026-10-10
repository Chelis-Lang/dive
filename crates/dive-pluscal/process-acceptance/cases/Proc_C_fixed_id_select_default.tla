---- MODULE Proc_C_fixed_id_select_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables g=0; process (P =1) variables x=<<0,0>>;  { L: x[1]:=self; E: g:=x[1]; } } *)
====
