---- MODULE MultiExtra_P_mixed_parallel ----
EXTENDS Naturals,TLC
(* --algorithm A variable g=0;  process P \in{1,2} variable x=0; begin L: x:=1 || g:=(x+1)%3; LE: skip; end process;  process Q =3 variable y=0; begin M: y:=2 || g:=(y+1)%3; ME: skip; end process;  process R \in{4,5} variable z=0; begin N: z:=3 || g:=(z+1)%3; NE: skip; end process; end algorithm; *)
====
