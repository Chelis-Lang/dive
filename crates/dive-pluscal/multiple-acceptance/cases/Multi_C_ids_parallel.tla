---- MODULE Multi_C_ids_parallel ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0;  process (P =1) variable x=0; { L: x:=1 || g:=2; E: skip; }  process (Q =2) variable y=0; { M: y:=2 || g:=1; F: skip; } } *)
====
