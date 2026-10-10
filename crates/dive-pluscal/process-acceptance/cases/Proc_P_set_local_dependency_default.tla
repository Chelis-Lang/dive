---- MODULE Proc_P_set_local_dependency_default ----
EXTENDS Naturals, TLC
(* --algorithm A variables g=0; process P \in{1,2} variables x=0,y=x+1; begin L: g:=y; E: x:=g; end process; end algorithm; *)
====
