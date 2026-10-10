---- MODULE ProcExtra_C_id_subscript ----
EXTENDS Naturals,TLC
(* --algorithm A { variables g=0; process (P =1) variables x = <<0,0>>; { L: x[1]:=1; E: g:=x[1]; } } *)
====
