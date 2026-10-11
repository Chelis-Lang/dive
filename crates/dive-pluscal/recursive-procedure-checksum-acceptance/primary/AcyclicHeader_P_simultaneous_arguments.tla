---- MODULE AcyclicHeader_P_simultaneous_arguments ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variables x=0;
procedure Outer(p=0)
begin O1:call Inner(p+1,p); O2:return; end procedure;
procedure Inner(q=0,r=0)
begin I1:x:=q+r; I2:return; end procedure;
begin L:call Outer(1); E:skip; end algorithm; *)
====
