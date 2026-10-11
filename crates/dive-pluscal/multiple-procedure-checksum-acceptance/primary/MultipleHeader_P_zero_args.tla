---- MODULE MultipleHeader_P_zero_args ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variable x=0; procedure Put()  begin P1:x:=3; P2:return; end procedure;procedure Get(q=0,b=0)  begin G1:x:=b+q; G2:return; end procedure; begin L:call Put(); M:call Get(3,4); E:skip; end algorithm; *)
====
