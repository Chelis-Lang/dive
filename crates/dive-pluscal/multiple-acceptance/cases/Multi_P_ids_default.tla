---- MODULE Multi_P_ids_default ----
EXTENDS Naturals,TLC
(* --algorithm A variable g=0;  process P =1 variable x; begin L: x:=1; E: g:=x; end process;  process Q =2 variable y; begin M: y:=2; F: g:=y; end process; end algorithm; *)
====
