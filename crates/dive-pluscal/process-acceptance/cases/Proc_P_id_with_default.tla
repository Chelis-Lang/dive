---- MODULE Proc_P_id_with_default ----
EXTENDS Naturals, TLC
(* --algorithm A variables g=0; process P =1 variables x=0; begin L: with k\in{0,1} do x:=k; end with; E: g:=x; end process; end algorithm; *)
====
