---- MODULE Multi_C_empty_id ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0;  process (P \in{}) variable x=0; { L: x:=1; E: g:=x; }  process (Q =1) variable y=0; { M: y:=2; F: g:=y; } } *)
====
