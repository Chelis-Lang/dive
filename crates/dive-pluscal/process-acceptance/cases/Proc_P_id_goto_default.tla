---- MODULE Proc_P_id_goto_default ----
EXTENDS Naturals, TLC
(* --algorithm A variables g=0; process P =1 variables x=0; begin L: x:=1; goto E; E: g:=x; end process; end algorithm; *)
====
