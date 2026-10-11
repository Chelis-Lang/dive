---- MODULE MultipleHeader_P_with_second ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variable x=0; procedure Put(z=0,a=0)  begin P1:x:=a+z; P2:return; end procedure;procedure Get(q=0,b=0)  begin G1:x:=b+q; G2:return; end procedure; begin L:with t=2 do call Get(t,1); goto E; end with; E:skip; end algorithm; *)
====
