---- MODULE Multi_C_nonliteral ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0;  process (P =1) variable x=0; { L: x:=1; E: g:=x; }  process (Q \in1..g) variable y=0; { M: y:=2; F: g:=y; } } *)
====
