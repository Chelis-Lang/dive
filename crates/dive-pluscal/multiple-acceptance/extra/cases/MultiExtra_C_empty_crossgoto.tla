---- MODULE MultiExtra_C_empty_crossgoto ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0;  process (P \in{}) variable x=0; { L: goto N; }  process (Q =3) variable y=0; { M: y:=2; ME: g:=y; }  process (R \in{}) variable z=0; { N: z:=3; NE: g:=z; } } *)
====
