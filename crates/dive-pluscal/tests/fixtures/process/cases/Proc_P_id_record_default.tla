---- MODULE Proc_P_id_record_default ----
EXTENDS Naturals, TLC
(* --algorithm A variables g=0; process P =1 variables x=[a|->0]; begin L: x.a:=self; E: g:=x.a; end process; end algorithm; *)
====
