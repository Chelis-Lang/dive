---- MODULE Multi_C_localcollision ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0;  process (P =1) variable x=0; { L: x:=1; E: g:=x; }  process (Q =2) variable x=0; { M: x:=2; F: g:=x; } } *)
====
