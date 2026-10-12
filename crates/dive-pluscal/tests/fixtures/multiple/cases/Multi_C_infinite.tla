---- MODULE Multi_C_infinite ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0;  process (P =1) variable x=0; { L: while (TRUE) { x:=(x+1)%2; }; }  process (Q =2) variable y=0; { M: while (TRUE) { y:=(y+1)%2; }; } } *)
====
