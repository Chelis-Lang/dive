---- MODULE MultipleHeader_P_locals ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variable x=0; procedure Put(z=0,a=0) variable u=0; begin P1:u:=a+z; P2:x:=u; P3:return; end procedure;procedure Get(q=0,b=0) variable v=0; begin G1:v:=b+q; G2:x:=v; G3:return; end procedure; begin L:call Put(1,2); M:call Get(3,4); E:skip; end algorithm; *)
====
