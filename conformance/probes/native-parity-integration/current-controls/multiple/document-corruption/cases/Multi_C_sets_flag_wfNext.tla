---- MODULE Multi_C_sets_flag_wfNext ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0;  process (P \in{1,2}) variable x=0; { L: x:=1; E: g:=x; }  process (Q \in{3,4}) variable y=0; { M: y:=2; F: g:=y; } } *)
====
