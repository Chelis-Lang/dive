---- MODULE Multi_C_processcollision ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0;  process (P =1) variable Q=0; { L: Q:=1; }  process (Q =2) variable y=0; { M: y:=2; F: g:=y; } } *)
====
