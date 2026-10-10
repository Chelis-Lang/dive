---- MODULE Multi_C_ids_default ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0;  process (P =1) variable x; { L: x:=1; E: g:=x; }  process (Q =2) variable y; { M: y:=2; F: g:=y; } } *)
====
