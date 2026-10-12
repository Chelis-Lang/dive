---- MODULE AcyclicHeader_P_chain ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variables x=0;
procedure Outer(p=0)
begin O1:call Inner(p+1); O2:x:=p; O3:return; end procedure;
procedure Inner(q=0)
begin I1:call Leaf(q+1); I2:return; end procedure;
procedure Leaf(r=0)
begin J1:x:=r; J2:return; end procedure;
begin L:call Outer(1); E:skip; end algorithm; *)
====
