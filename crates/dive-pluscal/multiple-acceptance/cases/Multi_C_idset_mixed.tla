---- MODULE Multi_C_idset_mixed ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0; fair process (P =1) variable x=0; { L: x:=1; E: g:=x; } fair + process (Q \in{2,3}) variable y=0; { M: y:=2; F: g:=y; } } *)
====
