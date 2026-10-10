---- MODULE MultiExtra_P_mixed_select ----
EXTENDS Naturals,TLC
(* --algorithm A variable g=0;  process P \in{1,2} variable x=[a |-> 0]; begin L: x.a:=1; LE: g:=x.a; end process;  process Q =3 variable y=[a |-> 0]; begin M: y.a:=2; ME: g:=y.a; end process;  process R \in{4,5} variable z=[a |-> 0]; begin N: z.a:=3; NE: g:=z.a; end process; end algorithm; *)
====
