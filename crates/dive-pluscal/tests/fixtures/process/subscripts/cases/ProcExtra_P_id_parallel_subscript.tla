---- MODULE ProcExtra_P_id_parallel_subscript ----
EXTENDS Naturals,TLC
(* --algorithm A variables g=0; process P =1 variables x = <<0,0>>; begin L: x[1]:=1 || x[2]:=2; E: g:=x[1]+x[2]; end process; end algorithm; *)
====
