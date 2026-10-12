---- MODULE Proc_P_id_if_default ----
EXTENDS Naturals, TLC
(* --algorithm A variables g=0; process P =1 variables x=0; begin L: if x=0 then x:=1; else g:=2; end if; E: skip; end process; end algorithm; *)
====
