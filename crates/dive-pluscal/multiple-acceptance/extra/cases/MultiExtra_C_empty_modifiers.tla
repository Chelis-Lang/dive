---- MODULE MultiExtra_C_empty_modifiers ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0; fair process (P \in{}) variable x=0; { L: + x:=1; LE: - g:=x; } fair process (Q =3) variable y=0; { M: + y:=2; ME: - g:=y; } fair process (R \in{}) variable z=0; { N: + z:=3; NE: - g:=z; } } *)
====
