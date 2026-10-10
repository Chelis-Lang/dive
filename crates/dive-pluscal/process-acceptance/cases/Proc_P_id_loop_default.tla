---- MODULE Proc_P_id_loop_default ----
EXTENDS Naturals, TLC
(* --algorithm A variables g=0; process P =1 variables x=0; begin L: while x<2 do x:=x+1; end while; E: g:=x; end process; end algorithm; *)
====
