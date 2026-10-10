---- MODULE MultiBound_P_member ----
EXTENDS Naturals,TLC
(* --algorithm A variable g=0; process P \in{1,2} variable x \in{0,1}; begin L: x:=1; E: g:=x; end process; process Q \in{3,4} variable y \in{0,1}; begin M: y:=2; F: g:=y; end process; end algorithm; *)
====
