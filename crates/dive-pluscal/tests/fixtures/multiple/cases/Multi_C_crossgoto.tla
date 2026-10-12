---- MODULE Multi_C_crossgoto ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0;  process (P =1) variable x=0; { L: goto M; }  process (Q =2) variable y=0; { M: y:=2; F: g:=y; } } *)
====
