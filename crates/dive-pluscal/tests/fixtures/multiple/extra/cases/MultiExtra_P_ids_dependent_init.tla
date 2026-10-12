---- MODULE MultiExtra_P_ids_dependent_init ----
EXTENDS Naturals,TLC
(* --algorithm A variable g=0;  process P =1 variables x=0, xd=x+1; begin L: x:=1; LE: g:=x; end process;  process Q =2 variables y=0, yd=y+1; begin M: y:=2; ME: g:=y; end process;  process R =3 variables z=0, zd=z+1; begin N: z:=3; NE: g:=z; end process; end algorithm; *)
====
