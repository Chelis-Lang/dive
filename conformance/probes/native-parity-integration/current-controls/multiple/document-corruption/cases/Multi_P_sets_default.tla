---- MODULE Multi_P_sets_default ----
EXTENDS Naturals,TLC
(* --algorithm A variable g=0;  process P \in{1,2} variable x; begin L: x:=1; E: g:=x; end process;  process Q \in{3,4} variable y; begin M: y:=2; F: g:=y; end process; end algorithm; *)
====
