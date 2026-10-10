---- MODULE Proc_P_id_self_default ----
EXTENDS Naturals, TLC
(* --algorithm A variables g=0; process P =1 variables x=self; begin L: g:=x; E: x:=self+1; end process; end algorithm; *)
====
