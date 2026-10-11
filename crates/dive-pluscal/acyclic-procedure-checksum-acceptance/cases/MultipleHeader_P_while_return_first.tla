---- MODULE MultipleHeader_P_while_return_first ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variable x=0; procedure Put(z=0,a=0)  begin P1:while x<0 do return; end while; P2:return; end procedure;procedure Get(q=0,b=0)  begin G1:x:=b+q; G2:return; end procedure; begin L:call Put(1,2); M:call Get(3,4); E:skip; end algorithm; *)
====
