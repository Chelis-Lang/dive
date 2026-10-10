---- MODULE MultiExtra_P_mixed_if ----
EXTENDS Naturals,TLC
(* --algorithm A variable g=0;  process P \in{1,2} variable x=0; begin L: if x=0 then x:=1; else x:=0; end if; LE: g:=x; end process;  process Q =3 variable y=0; begin M: if y=0 then y:=2; else y:=0; end if; ME: g:=y; end process;  process R \in{4,5} variable z=0; begin N: if z=0 then z:=3; else z:=0; end if; NE: g:=z; end process; end algorithm; *)
====
