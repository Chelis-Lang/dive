---- MODULE Multi_C_sets_parallel ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0;  process (P \in{1,2}) variable x=0; { L: x:=1 || g:=2; E: skip; }  process (Q \in{3,4}) variable y=0; { M: y:=2 || g:=1; F: skip; } } *)
====
