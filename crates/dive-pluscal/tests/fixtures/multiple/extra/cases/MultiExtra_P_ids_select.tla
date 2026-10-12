---- MODULE MultiExtra_P_ids_select ----
EXTENDS Naturals,TLC
(* --algorithm A variable g=0;  process P =1 variable x=[a |-> 0]; begin L: x.a:=1; LE: g:=x.a; end process;  process Q =2 variable y=[a |-> 0]; begin M: y.a:=2; ME: g:=y.a; end process;  process R =3 variable z=[a |-> 0]; begin N: z.a:=3; NE: g:=z.a; end process; end algorithm; *)
====
