---- MODULE MultiExtra_C_ids_parallel ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0;  process (P =1) variable x=0; { L: x:=1 || g:=(x+1)%3; LE: skip; }  process (Q =2) variable y=0; { M: y:=2 || g:=(y+1)%3; ME: skip; }  process (R =3) variable z=0; { N: z:=3 || g:=(z+1)%3; NE: skip; } } *)
====
