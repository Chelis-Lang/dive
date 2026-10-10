---- MODULE MultiBound_P_escaped_overlap ----
EXTENDS Naturals,TLC
(* --algorithm A variable g=0; process P ="a\tb" variable x=0; begin L: x:=1; E: g:=x; end process; process Q ="a\tb" variable y=0; begin M: y:=2; F: g:=y; end process; end algorithm; *)
====
