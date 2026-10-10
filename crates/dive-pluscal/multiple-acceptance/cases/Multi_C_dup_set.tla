---- MODULE Multi_C_dup_set ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0;  process (P \in{1,1}) variable x=0; { L: x:=1; E: g:=x; }  process (Q \in{2,2}) variable y=0; { M: y:=2; F: g:=y; } } *)
====
