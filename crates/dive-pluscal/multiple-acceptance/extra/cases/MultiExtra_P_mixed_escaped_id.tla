---- MODULE MultiExtra_P_mixed_escaped_id ----
EXTENDS Naturals,TLC
(* --algorithm A variable g=0;  process P \in{1,2} variable x=0; begin L: x:=1; LE: g:=x; end process;  process Q =3 variable y=0; begin M: y:=2; ME: g:=y; end process;  process R ="a\tb" variable z=0; begin N: z:=3; NE: g:=z; end process; end algorithm; *)
====
