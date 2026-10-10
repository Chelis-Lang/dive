---- MODULE Multi_C_crosslocal ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0;  process (P =1) variable x=0; { L: x:=y; }  process (Q =2) variable y=0; { M: y:=2; F: g:=y; } } *)
====
