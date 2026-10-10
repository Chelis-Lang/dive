---- MODULE Multi_P_idset_parallel ----
EXTENDS Naturals,TLC
(* --algorithm A variable g=0;  process P =1 variable x=0; begin L: x:=1 || g:=2; E: skip; end process;  process Q \in{2,3} variable y=0; begin M: y:=2 || g:=1; F: skip; end process; end algorithm; *)
====
