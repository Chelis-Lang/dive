---- MODULE MultiExtra_P_ids_either ----
EXTENDS Naturals,TLC
(* --algorithm A variable g=0;  process P =1 variable x=0; begin L: either x:=1; or x:=2; end either; LE: g:=x; end process;  process Q =2 variable y=0; begin M: either y:=1; or y:=2; end either; ME: g:=y; end process;  process R =3 variable z=0; begin N: either z:=1; or z:=2; end either; NE: g:=z; end process; end algorithm; *)
====
