---- MODULE MultipleHeader_P_if_first_else_second ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variable x=0; procedure Put(z=0,a=0)  begin P1:x:=a+z; P2:return; end procedure;procedure Get(q=0,b=0)  begin G1:x:=b+q; G2:return; end procedure; begin L:if x=0 then call Put(1,2); goto E; else call Get(3,4); goto E; end if; E:skip; end algorithm; *)
====
