---- MODULE Multi_P_sets_modifiers ----
EXTENDS Naturals,TLC
(* --algorithm A variable g=0; fair process P \in{1,2} variable x=0; begin L: + x:=1; E: - g:=x; end process; fair process Q \in{3,4} variable y=0; begin M: - y:=2; F: + g:=y; end process; end algorithm; *)
====
