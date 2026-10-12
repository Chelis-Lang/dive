---- MODULE Multi_C_idset_default ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0;  process (P =1) variable x; { L: x:=1; E: g:=x; }  process (Q \in{2,3}) variable y; { M: y:=2; F: g:=y; } } *)
====
