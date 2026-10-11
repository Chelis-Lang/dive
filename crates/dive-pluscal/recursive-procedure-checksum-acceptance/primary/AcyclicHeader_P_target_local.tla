---- MODULE AcyclicHeader_P_target_local ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variables x=0;
procedure Outer(p=0)
begin O1:call Inner(p+1); O2:x:=p; O3:return; end procedure;
procedure Inner(q=0)
variables b=0;
begin I1:b:=q+1; I2:x:=b; I3:return; end procedure;
begin L:call Outer(1); E:skip; end algorithm; *)
====
