---- MODULE Proc_P_set_self_default ----
EXTENDS Naturals, TLC
(* --algorithm A variables g=0; process P \in{1,2} variables x=self; begin L: g:=x; E: x:=self+1; end process; end algorithm; *)
====
