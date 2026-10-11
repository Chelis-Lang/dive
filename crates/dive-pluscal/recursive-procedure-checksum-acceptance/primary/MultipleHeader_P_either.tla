---- MODULE MultipleHeader_P_either ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variable x=0; procedure Put(z=0,a=0)  begin P1:x:=a+z; P2:return; end procedure;procedure Get(q=0,b=0)  begin G1:x:=b+q; G2:return; end procedure; begin L:either call Put(1,2); goto E; or call Get(3,4); goto E; end either; E:skip; end algorithm; *)
====
