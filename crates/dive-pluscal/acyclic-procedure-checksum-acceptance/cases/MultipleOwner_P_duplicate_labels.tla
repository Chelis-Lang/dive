---- MODULE MultipleOwner_P_duplicate_labels ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variable x=0; procedure Put(z=0,a=0)  begin P1:x:=a+z; P2:return; end procedure;procedure Get(q=0,b=0)  begin P1:x:=b+q; P2:return; end procedure; begin L:call Put(1,2); M:call Get(3,4); E:skip; end algorithm; *)
====
