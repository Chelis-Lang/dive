---- MODULE MultiExtra_C_sets_parallel ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0;  process (P \in{1,2}) variable x=0; { L: x:=1 || g:=(x+1)%3; LE: skip; }  process (Q \in{3,4}) variable y=0; { M: y:=2 || g:=(y+1)%3; ME: skip; }  process (R \in{5}) variable z=0; { N: z:=3 || g:=(z+1)%3; NE: skip; } } *)
====
