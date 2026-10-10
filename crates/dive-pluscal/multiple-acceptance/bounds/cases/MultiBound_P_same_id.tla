---- MODULE MultiBound_P_same_id ----
EXTENDS Naturals,TLC
(* --algorithm A variable g=0; process P =1 variable x=0; begin L: x:=1; E: g:=x; end process; process Q =1 variable y=0; begin M: y:=2; F: g:=y; end process; end algorithm; *)
====
