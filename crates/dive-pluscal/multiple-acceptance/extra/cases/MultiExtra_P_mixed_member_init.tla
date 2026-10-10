---- MODULE MultiExtra_P_mixed_member_init ----
EXTENDS Naturals,TLC
(* --algorithm A variable g=0;  process P \in{1,2} variable x \in {0,1}; begin L: x:=1; LE: g:=x; end process;  process Q =3 variable y \in {0,1}; begin M: y:=2; ME: g:=y; end process;  process R \in{4,5} variable z \in {0,1}; begin N: z:=3; NE: g:=z; end process; end algorithm; *)
====
