---- MODULE Multi_P_empty_id ----
EXTENDS Naturals,TLC
(* --algorithm A variable g=0;  process P \in{} variable x=0; begin L: x:=1; E: g:=x; end process;  process Q =1 variable y=0; begin M: y:=2; F: g:=y; end process; end algorithm; *)
====
