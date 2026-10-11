---- MODULE MultipleHeader_P_either_return_second ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variable x=0; procedure Put(z=0,a=0)  begin P1:x:=a+z; P2:return; end procedure;procedure Get(q=0,b=0)  begin G1:either return; or x:=b; return; end either; G2:skip; end procedure; begin L:call Put(1,2); M:call Get(3,4); E:skip; end algorithm; *)
====
