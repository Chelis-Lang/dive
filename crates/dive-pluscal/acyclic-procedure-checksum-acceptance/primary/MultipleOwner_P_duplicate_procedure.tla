---- MODULE MultipleOwner_P_duplicate_procedure ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variable x=0; procedure Put(z=0,a=0)  begin P1:x:=a+z; P2:return; end procedure;procedure Put(q=0,b=0)  begin G1:x:=b+q; G2:return; end procedure; begin L:call Put(1,2); M:call Put(3,4); E:skip; end algorithm; *)
====
