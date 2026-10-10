---- MODULE Multi_C_mixed ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0;  process (P =1) variable x=0; { L: x:=1; E: g:=x; }  process (Q ="q") variable y=0; { M: y:=2; F: g:=y; } } *)
====
