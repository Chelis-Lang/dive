---- MODULE MultipleHeader_P_with_return_first ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variable x=0; procedure Put(z=0,a=0)  begin P1:with t=a do x:=t; return; end with; P2:skip; end procedure;procedure Get(q=0,b=0)  begin G1:x:=b+q; G2:return; end procedure; begin L:call Put(1,2); M:call Get(3,4); E:skip; end algorithm; *)
====
