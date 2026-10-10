---- MODULE MultiExtra_C_sets_crossgoto ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0;  process (P \in{1,2}) variable x=0; { L: goto N; }  process (Q \in{3,4}) variable y=0; { M: y:=2; ME: g:=y; }  process (R \in{5}) variable z=0; { N: z:=3; NE: g:=z; } } *)
====
