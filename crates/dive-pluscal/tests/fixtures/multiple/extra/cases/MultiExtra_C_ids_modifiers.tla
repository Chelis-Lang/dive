---- MODULE MultiExtra_C_ids_modifiers ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0; fair process (P =1) variable x=0; { L: + x:=1; LE: - g:=x; } fair process (Q =2) variable y=0; { M: + y:=2; ME: - g:=y; } fair process (R =3) variable z=0; { N: + z:=3; NE: - g:=z; } } *)
====
