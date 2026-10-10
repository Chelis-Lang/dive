---- MODULE Proc_P_id_local_dependency_default ----
EXTENDS Naturals, TLC
(* --algorithm A variables g=0; process P =1 variables x=0,y=x+1; begin L: g:=y; E: x:=g; end process; end algorithm; *)
====
