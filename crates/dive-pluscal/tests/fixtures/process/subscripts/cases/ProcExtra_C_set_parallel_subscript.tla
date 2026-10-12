---- MODULE ProcExtra_C_set_parallel_subscript ----
EXTENDS Naturals,TLC
(* --algorithm A { variables g=0; process (P \in{1,2}) variables x = <<0,0>>; { L: x[1]:=1 || x[2]:=2; E: g:=x[1]+x[2]; } } *)
====
