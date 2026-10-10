---- MODULE Multi_P_idset ----
EXTENDS Naturals,TLC
(* --algorithm A variable g=0;  process P =1 variable x=0; begin L: x:=1; E: g:=x; end process;  process Q \in{2,3} variable y=0; begin M: y:=2; F: g:=y; end process; end algorithm; *)
====
