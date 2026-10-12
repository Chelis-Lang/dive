---- MODULE MultipleHeader_P_tuple_argument ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variable x=0; procedure Put(a=0)  begin P1:x:=a[1]; P2:return; end procedure;procedure Get(q=0,b=0)  begin G1:x:=b+q; G2:return; end procedure; begin L:call Put(<<3>>); M:call Get(3,4); E:skip; end algorithm; *)
====
