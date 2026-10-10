---- MODULE Proc_P_set_parallel_default ----
EXTENDS Naturals, TLC
(* --algorithm A variables g=0; process P \in{1,2} variables x=0,y=0; begin L: x:=y+1 || y:=x+1; E: g:=x+y; end process; end algorithm; *)
====
