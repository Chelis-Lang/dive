---- MODULE MultiBound_C_reversed_both ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0; process (P \in3..1) variable x=0; { L: x:=1; E: g:=x; } process (Q \in4..2) variable y=0; { M: y:=2; F: g:=y; } } *)
====
